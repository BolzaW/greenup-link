//! Low-level UART/USB serial port driver for the ATmega2560 power board.
//!
//! Responsibilities:
//! - USB serial port discovery (`discover_candidate_ports`) and worker spawning (`spawn_port`).
//! - Framed line reading (`\r\n`) with OOM/noise protection (`MAX_SERIAL_LINE_LEN`) and automatic reconnection.
//! - Synchronous TX command transmission with ACK matching and retry (`send_command_sync`).
//! - Hardware initialization handshake (`RaspberryPiModeOK` + `Command::startup_queries()`).
//! - Linky TIC baudrate detection sequence (`TICTM:1` -> `TICTestB:` -> `TICTM:0`).
//! - Watchdog keepalive (`ping` / `pong`) and link health updates (`UsbLinkState`).
//! - Updating `AtmegaRawState` and broadcasting `DriverSnapshot` on `tokio::sync::watch`.

use crate::logger;
use crate::raw_state::{AtmegaRawState, SerialStatus};
use crate::state::{DriverHandle, SharedDriver};
use greenup_protocol::{
    commands::FunctioningMode,
    parser::{parse_line, ProtocolEvent},
    Command,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Fixed hardware UART baud rate of the Legrand ATmega2560 power board (115200 bps, 8N1).
pub const ATMEGA_BAUD_RATE: u32 = 115_200;

#[derive(Clone, Debug)]
pub struct SerialOptions {
    pub port: String,
    pub baud_rate: u32,
    pub timeout_ms: u64,
    pub watchdog_timeout_secs: u64,
}

impl Default for SerialOptions {
    fn default() -> Self {
        Self {
            port: "/dev/ttyUSB0".to_string(),
            baud_rate: ATMEGA_BAUD_RATE,
            timeout_ms: 100,
            watchdog_timeout_secs: 60,
        }
    }
}

/// Maximum allowed length in bytes for a single line from the serial port (256 bytes).
/// Protects against OOM caused by electrical noise on the UART bus streaming bytes without delimiters.
pub const MAX_SERIAL_LINE_LEN: usize = 256;

const TICTM_MAX_ZEROS: u32 = 5;

/// Discovers candidate USB serial ports for Point 1 and Point 2 on the host OS.
pub fn discover_candidate_ports(
    configured_port: Option<&str>,
    configured_port2: Option<&str>,
) -> (String, String) {
    let mut available: Vec<String> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.port_name)
        .filter(|name| {
            name.contains("ttyUSB")
                || name.contains("ttyACM")
                || name.starts_with("COM")
                || name.contains("cu.usbserial")
        })
        .collect();

    available.sort();

    let port1 = match configured_port {
        Some(p) if !p.trim().is_empty() => p.trim().to_string(),
        _ => available
            .first()
            .cloned()
            .unwrap_or_else(|| "/dev/ttyUSB0".to_string()),
    };

    let port2 = match configured_port2 {
        Some(p2) if !p2.trim().is_empty() => p2.trim().to_string(),
        _ => available
            .iter()
            .find(|&p| p != &port1)
            .cloned()
            .unwrap_or_else(|| {
                if port1.ends_with('0') {
                    format!("{}1", &port1[..port1.len() - 1])
                } else {
                    "/dev/ttyUSB1".to_string()
                }
            }),
    };

    (port1, port2)
}

/// Spawns a new low-level USB serial driver thread for the given port and returns its [`SharedDriver`] handle.
/// Can be called at startup for Point 1, and dynamically by `greenup-link` when initializing a 2nd USB port.
pub fn spawn_port(side: u8, sniffer_mode: bool, options: SerialOptions) -> SharedDriver {
    let (tx, rx) = tokio::sync::mpsc::channel::<Command>(64);
    let driver = Arc::new(DriverHandle::new_with_side_and_sniffer(
        side,
        sniffer_mode,
        tx,
    ));
    driver.update_usb(|usb| {
        usb.port = options.port.clone();
        usb.baud_rate = options.baud_rate;
        usb.message = format!("Initializing USB serial port ({})", options.port);
    });

    let driver_for_loop = driver.clone();
    std::thread::spawn(move || {
        run_serial_loop_with_options(driver_for_loop, rx, options);
    });

    driver
}

/// Queries executed by the driver during the initialization sequence, matching the registers
/// tracked in [`AtmegaRawState`] (excludes Sleep, EM, RFID, and OCPP config commands,
/// while keeping `ModbusEMA?`, `ModbusDLMA?`, and `OCPPPS?`).
pub fn driver_startup_queries() -> Vec<Command> {
    Command::startup_queries()
        .into_iter()
        .filter(|cmd| {
            !matches!(
                cmd,
                Command::GetSleep
                    | Command::GetEnergyMeterPulses
                    | Command::GetRfidEnabled
                    | Command::GetRfidUserManagement
            )
        })
        .collect()
}

/// Triggers the low-level ATmega initialization sequence (`RaspberryPiModeOK` + [`driver_startup_queries`]).
pub async fn trigger_init_sequence(driver: &SharedDriver) {
    let _ = driver.serial_tx.send(Command::RaspberryPiModeOk).await;

    for cmd in driver_startup_queries() {
        let _ = driver.serial_tx.send(cmd).await;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum TicStartError {
    /// The charging station hardware model does not support TIC (checked by upper layer).
    NotSupported,
    /// The charging station is not in `State:A` (contains the current Legrand state).
    NotInStateA(String),
    /// The channel to the serial port is closed.
    Serial,
}

/// Low-level check verifying that the ATmega is currently in `State:A` (Free / Available)
/// before activating `TICTM:1`.
pub fn check_tic_state_a(driver: &SharedDriver) -> Result<(), TicStartError> {
    let current_state = driver
        .get_raw_state()
        .state
        .unwrap_or_else(|| String::from("Unknown"));

    if current_state != "A" {
        return Err(TicStartError::NotInStateA(current_state));
    }

    Ok(())
}

/// Starts the low-level Linky TIC baudrate detection sequence (`TICTM:1`),
/// provided the ATmega is currently in `State:A`.
///
/// The driver automatically sends `TICTM:0` once a non-zero baudrate (`1200`/`9600`)
/// or `TICTM_MAX_ZEROS` consecutive `"0"` frames are received.
pub async fn start_tic_detection(driver: &SharedDriver) -> Result<(), TicStartError> {
    match check_tic_state_a(driver) {
        Ok(()) => {}
        Err(TicStartError::NotInStateA(ref current_state)) => {
            logger::log(
                "SERIAL",
                &format!(
                    "⛔ TIC detection ignored: charging station not free (State: {})",
                    current_state
                ),
            );
            return Err(TicStartError::NotInStateA(current_state.clone()));
        }
        Err(e) => return Err(e),
    }

    logger::debug("SERIAL", "🚀 Starting TIC detection (TICTM:1)");
    if let Ok(mut tic) = driver.tic_detection.lock() {
        tic.is_active = true;
        tic.zero_count = 0;
    }
    driver.update_raw(|raw| {
        raw.tic_tm = Some(true);
    });

    driver
        .serial_tx
        .send(Command::SetTicTestMode(true))
        .await
        .map_err(|_| TicStartError::Serial)
}

/// Sends a command synchronously to the serial port, logging TX, and waiting
/// for the matching acknowledgment response from the ATmega if the command expects one.
pub fn send_command_sync(
    port: &mut Box<dyn serialport::SerialPort>,
    driver: &SharedDriver,
    internal_rx: &std::sync::mpsc::Receiver<String>,
    is_running: &AtomicBool,
    cmd: Command,
) -> bool {
    let encoded = cmd.encode();
    let expects_response = cmd.expects_response();
    let mut retries = if cmd == Command::Ping { 0 } else { 1 };
    let mut success = false;

    while retries >= 0 && !success && is_running.load(Ordering::Relaxed) {
        while internal_rx.try_recv().is_ok() {}

        let tx_level = if cmd == Command::Ping {
            logger::LogLevel::Trace
        } else {
            logger::LogLevel::Info
        };
        logger::log_with_level_side("SERIAL_TX", Some(driver.side()), tx_level, encoded.trim());

        if let Err(e) = port.write_all(encoded.as_bytes()) {
            logger::error("SERIAL", &format!("❌ Serial write error on port: {}", e));
            is_running.store(false, Ordering::Relaxed);
            return false;
        }

        driver.update_usb(|usb| {
            usb.tx_commands_count = usb.tx_commands_count.wrapping_add(1);
        });

        if expects_response {
            let start_wait = Instant::now();
            let mut matched = false;
            let mut got_default = false;

            while start_wait.elapsed() < Duration::from_millis(1000)
                && is_running.load(Ordering::Relaxed)
            {
                if let Ok(rx_line) = internal_rx.recv_timeout(Duration::from_millis(50)) {
                    if cmd.matches_response(&rx_line) {
                        matched = true;
                        break;
                    } else if rx_line.starts_with("Default:") {
                        got_default = true;
                        break;
                    }
                }
            }

            if matched {
                success = true;
            } else {
                if got_default {
                    logger::warn(
                        "SERIAL",
                        &format!(
                            "❌ Command {} rejected by station (Default), retry: {}",
                            cmd.as_frame(),
                            retries
                        ),
                    );
                } else {
                    logger::warn(
                        "SERIAL",
                        &format!(
                            "⏳ Command {} timeout, retry: {}",
                            cmd.as_frame(),
                            retries
                        ),
                    );
                }
                retries -= 1;
            }
        } else {
            success = true;
        }
    }

    success
}

/// Executes the synchronous startup handshake (`RaspberryPiModeOK` + `Command::startup_queries()`)
/// and notifies upper layers via `usb.init_completed = true`.
pub fn send_init_sequence_sync(
    port: &mut Box<dyn serialport::SerialPort>,
    driver: &SharedDriver,
    internal_rx: &std::sync::mpsc::Receiver<String>,
    is_running: &AtomicBool,
) {
    driver.update_usb(|usb| {
        usb.init_completed = false;
    });

    send_command_sync(
        port,
        driver,
        internal_rx,
        is_running,
        Command::RaspberryPiModeOk,
    );

    for cmd in driver_startup_queries() {
        if !is_running.load(Ordering::Relaxed) {
            break;
        }
        send_command_sync(port, driver, internal_rx, is_running, cmd);
    }

    if is_running.load(Ordering::Relaxed) {
        driver.update_usb(|usb| {
            usb.init_completed = true;
        });
    }
}

pub fn run_serial_loop(driver: SharedDriver, rx_channel: tokio::sync::mpsc::Receiver<Command>) {
    run_serial_loop_with_options(driver, rx_channel, SerialOptions::default());
}

pub fn run_serial_loop_with_options(
    driver: SharedDriver,
    mut rx_channel: tokio::sync::mpsc::Receiver<Command>,
    options: SerialOptions,
) {
    let port_name = options.port;
    let baud_rate = options.baud_rate;
    let watchdog_timeout = Duration::from_secs(options.watchdog_timeout_secs);
    let mut last_logged_disconnect: Option<Instant> = None;

    loop {
        let mut port = match serialport::new(&port_name, baud_rate)
            .timeout(Duration::from_millis(options.timeout_ms))
            .open()
        {
            Ok(p) => p,
            Err(e) => {
                let should_log = match last_logged_disconnect {
                    None => true,
                    Some(t) => t.elapsed() >= Duration::from_secs(15),
                };

                if should_log {
                    logger::error(
                        "SERIAL",
                        &format!(
                            "❌ USB serial port not found or inaccessible ({}: {}). Check the USB cable connected to the Raspberry Pi. Retrying in 2s...",
                            port_name, e
                        ),
                    );
                    last_logged_disconnect = Some(Instant::now());
                }

                driver.update_both(|raw, usb| {
                    usb.status = SerialStatus::UsbDisconnected;
                    usb.port = port_name.clone();
                    usb.baud_rate = baud_rate;
                    usb.message = format!("USB cable not detected ({})", e);
                    usb.last_rx_secs_ago = None;
                    usb.init_completed = false;
                    raw.state = Some("DISCONNECTED".to_string());
                    raw.tic_tm = None;
                    raw.cci = Some(0.0);
                    raw.cci_l2 = None;
                    raw.cci_l3 = None;
                });

                while let Ok(cmd) = rx_channel.try_recv() {
                    logger::debug(
                        "SERIAL",
                        &format!("🗑️ Command {} dropped (USB disconnected)", cmd.as_frame()),
                    );
                }

                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        last_logged_disconnect = None;
        logger::info(
            "SERIAL",
            &format!(
                "🔌 USB serial port opened successfully on {} at {} baud. Initializing communication with ATmega...",
                port_name, baud_rate
            ),
        );

        driver.update_usb(|usb| {
            usb.status = SerialStatus::UsbConnecting;
            usb.port = port_name.clone();
            usb.baud_rate = baud_rate;
            usb.message =
                "USB port connected, waiting for response from station board".to_string();
            usb.last_rx_secs_ago = None;
            usb.init_completed = false;
        });

        let clone_port = match port.try_clone() {
            Ok(p) => p,
            Err(e) => {
                logger::error(
                    "SERIAL",
                    &format!("❌ Failed to clone serial port {}: {}", port_name, e),
                );
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        let is_running = Arc::new(AtomicBool::new(true));
        let last_rx_time = Arc::new(std::sync::Mutex::new(None::<Instant>));
        let (internal_tx, internal_rx) = std::sync::mpsc::channel::<String>();

        let is_running_reader = is_running.clone();
        let last_rx_reader = last_rx_time.clone();
        let driver_rx = driver.clone();
        let port_name_reader = port_name.clone();

        let reader_thread = std::thread::spawn(move || {
            let mut reader = BufReader::new(clone_port);
            let mut byte_buf = Vec::new();
            while is_running_reader.load(Ordering::Relaxed) {
                if byte_buf.len() >= MAX_SERIAL_LINE_LEN {
                    logger::warn(
                        "SERIAL",
                        &format!(
                            "⚠️ Serial buffer exceeded {} bytes without newline delimiter (noise or invalid baud). Discarding corrupted buffer.",
                            MAX_SERIAL_LINE_LEN
                        ),
                    );
                    byte_buf.clear();
                }

                let remaining = (MAX_SERIAL_LINE_LEN - byte_buf.len()) as u64;
                match (&mut reader).take(remaining).read_until(b'\n', &mut byte_buf) {
                    Ok(n) if n > 0 => {
                        if byte_buf.ends_with(b"\n") {
                            let clean_str = String::from_utf8_lossy(&byte_buf);
                            let clean_line = clean_str.trim();
                            if !clean_line.is_empty() {
                                if let Ok(mut t) = last_rx_reader.lock() {
                                    *t = Some(Instant::now());
                                }

                                {
                                    let mut usb = driver_rx
                                        .usb
                                        .lock()
                                        .unwrap_or_else(|e| e.into_inner());
                                    if usb.status != SerialStatus::UsbConnected {
                                        usb.status = SerialStatus::UsbConnected;
                                        usb.message =
                                            "USB serial link active - Station connected".to_string();
                                        logger::info(
                                            "SERIAL",
                                            &format!(
                                                "✅ USB serial communication established on {} (ATmega link active)",
                                                port_name_reader
                                            ),
                                        );
                                    }
                                    usb.last_rx_secs_ago = Some(0);
                                    usb.rx_frames_count = usb.rx_frames_count.wrapping_add(1);
                                }

                                parse_incoming_line(clean_line, &driver_rx);
                                let _ = internal_tx.send(clean_line.to_string());
                            }
                            byte_buf.clear();
                        } else if byte_buf.len() >= MAX_SERIAL_LINE_LEN {
                            logger::warn(
                                "SERIAL",
                                &format!(
                                    "⚠️ Serial buffer exceeded {} bytes without newline delimiter (noise or invalid baud). Discarding corrupted buffer.",
                                    MAX_SERIAL_LINE_LEN
                                ),
                            );
                            byte_buf.clear();
                        }
                    }
                    Ok(_) => {
                        logger::error(
                            "SERIAL",
                            &format!("🔌 USB cable disconnected (EOF on {}).", port_name_reader),
                        );
                        is_running_reader.store(false, Ordering::Relaxed);
                        break;
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(e) => {
                        logger::error(
                            "SERIAL",
                            &format!(
                                "🔌 I/O error on USB serial port {} : {}. Cable disconnected or link broken.",
                                port_name_reader, e
                            ),
                        );
                        is_running_reader.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });

        reset_serial_filters();
        if !driver.sniffer_mode() {
            send_init_sequence_sync(&mut port, &driver, &internal_rx, &is_running);
        } else {
            logger::info(
                "SERIAL",
                "👂 Operating in Sniffer mode: passive monitoring enabled. ATmega init sequence bypassed (zero serial transmission).",
            );
        }

        let session_start = Instant::now();
        let mut last_watchdog_probe = Instant::now();
        let mut last_watchdog_warn = Instant::now();
        let mut last_published_rx_secs: Option<u64> = None;

        while is_running.load(Ordering::Relaxed) {
            let cmd_opt = match rx_channel.try_recv() {
                Ok(cmd) => Some(cmd),
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    logger::warn("SERIAL", "Command channel closed (service shutting down)");
                    is_running.store(false, Ordering::Relaxed);
                    break;
                }
            };

            if let Some(cmd) = cmd_opt {
                if driver.sniffer_mode() {
                    logger::warn(
                        "SERIAL",
                        &format!(
                            "⛔ Command {} dropped: station operating in Sniffer mode (passive)",
                            cmd.as_frame()
                        ),
                    );
                } else {
                    send_command_sync(&mut port, &driver, &internal_rx, &is_running, cmd);
                }
            } else {
                std::thread::sleep(Duration::from_millis(50));
            }

            let (last_rx_opt, current_status) = {
                let rx_lock = last_rx_time.lock().unwrap_or_else(|e| e.into_inner());
                let usb_lock = driver.usb.lock().unwrap_or_else(|e| e.into_inner());
                (*rx_lock, usb_lock.status.clone())
            };

            let elapsed_since_rx = match last_rx_opt {
                Some(t) => t.elapsed(),
                None => session_start.elapsed(),
            };

            let secs_ago = elapsed_since_rx.as_secs();
            if last_published_rx_secs != Some(secs_ago) {
                last_published_rx_secs = Some(secs_ago);
                driver.update_usb(|usb| {
                    usb.last_rx_secs_ago = Some(secs_ago);
                });
            }

            let probe_threshold =
                Duration::from_secs(options.watchdog_timeout_secs.saturating_sub(15).max(5));

            if elapsed_since_rx >= probe_threshold && elapsed_since_rx < watchdog_timeout {
                if !driver.sniffer_mode()
                    && last_watchdog_probe.elapsed() >= Duration::from_secs(7)
                {
                    last_watchdog_probe = Instant::now();
                    send_command_sync(&mut port, &driver, &internal_rx, &is_running, Command::Ping);
                }
            } else if elapsed_since_rx >= watchdog_timeout {
                if current_status == SerialStatus::UsbConnected {
                    driver.update_usb(|usb| {
                        usb.status = SerialStatus::UsbNoResponse;
                        usb.message =
                            "No response from station board (station powered off or ATmega frozen)"
                                .to_string();
                    });
                    if last_watchdog_warn.elapsed() >= Duration::from_secs(15) {
                        logger::warn(
                            "SERIAL",
                            &format!(
                                "⚠️ No response from station board on {} for {}s (ping probes unanswered). Check station power or power-cycle station if ATmega is frozen.",
                                port_name,
                                elapsed_since_rx.as_secs()
                            ),
                        );
                        last_watchdog_warn = Instant::now();
                    }
                } else if current_status == SerialStatus::UsbConnecting
                    && elapsed_since_rx > Duration::from_secs(5)
                {
                    driver.update_usb(|usb| {
                        usb.status = SerialStatus::UsbNoResponse;
                        usb.message =
                            "No response from station board (station powered off or ATmega frozen)"
                                .to_string();
                    });
                    if last_watchdog_warn.elapsed() >= Duration::from_secs(15) {
                        logger::warn(
                            "SERIAL",
                            &format!(
                                "⏳ USB port open on {} but no response from ATmega after {}s. Check station power or power-cycle station if ATmega is frozen.",
                                port_name,
                                elapsed_since_rx.as_secs()
                            ),
                        );
                        last_watchdog_warn = Instant::now();
                    }
                }

                if !driver.sniffer_mode()
                    && last_watchdog_probe.elapsed() >= Duration::from_secs(15)
                {
                    last_watchdog_probe = Instant::now();
                    logger::warn(
                        "SERIAL",
                        &format!(
                            "🚨 Attempting emergency ATmega interface reset (RaspberryPiModeOK) on {}...",
                            port_name
                        ),
                    );
                    send_command_sync(
                        &mut port,
                        &driver,
                        &internal_rx,
                        &is_running,
                        Command::RaspberryPiModeOk,
                    );
                }
            }
        }

        is_running.store(false, Ordering::Relaxed);
        let _ = reader_thread.join();
        drop(port);

        logger::warn("SERIAL", "🔌 Serial session ended. Resuming USB search in 2s...");
        std::thread::sleep(Duration::from_secs(2));
    }
}

static LAST_LOGGED_CCI: std::sync::Mutex<Option<f32>> = std::sync::Mutex::new(None);
static LAST_LOGGED_CC: std::sync::Mutex<Option<u32>> = std::sync::Mutex::new(None);
static LAST_LOGGED_CPH: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// Resets serial logging filter baselines (e.g. on new charge session or connection reset).
pub fn reset_serial_filters() {
    *LAST_LOGGED_CCI.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *LAST_LOGGED_CC.lock().unwrap_or_else(|e| e.into_inner()) = None;
    *LAST_LOGGED_CPH.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Determines the logging category/level for an incoming serial line:
/// - `CCI:xx`: logged in INFO (`"SERIAL_RX"`) only if variation >= 0.8A (otherwise `"TRACE"`)
/// - `CC:xx`: logged in INFO (`"SERIAL_RX"`) only if variation >= 1A (otherwise `"TRACE"`)
/// - `CPh:xx`: logged in INFO (`"SERIAL_RX"`) on first reception or value change (otherwise `"DEBUG"`)
/// - Other frames: logged in INFO (`"SERIAL_RX"`)
pub fn determine_rx_log_category(line: &str) -> &'static str {
    if let Some(val_str) = line.strip_prefix("CCI:") {
        let first_val = val_str.split(';').next().unwrap_or(val_str).trim();
        if let Ok(current_val) = first_val.parse::<f32>() {
            let mut last = LAST_LOGGED_CCI.lock().unwrap_or_else(|e| e.into_inner());
            let diff = match *last {
                Some(prev) => (current_val - prev).abs(),
                None => 0.8,
            };
            if diff >= 0.8 {
                *last = Some(current_val);
                "SERIAL_RX"
            } else {
                "TRACE"
            }
        } else {
            "TRACE"
        }
    } else if let Some(val_str) = line.strip_prefix("CC:") {
        if let Ok(current_val) = val_str.trim().parse::<u32>() {
            let mut last = LAST_LOGGED_CC.lock().unwrap_or_else(|e| e.into_inner());
            let diff = match *last {
                Some(prev) => (current_val as i32 - prev as i32).abs(),
                None => 1,
            };
            if diff >= 1 {
                *last = Some(current_val);
                "SERIAL_RX"
            } else {
                "TRACE"
            }
        } else {
            "TRACE"
        }
    } else if let Some(val_str) = line
        .strip_prefix("CPh:")
        .or_else(|| line.strip_prefix("Cph:"))
    {
        let current_cph = val_str.trim();
        let mut last = LAST_LOGGED_CPH.lock().unwrap_or_else(|e| e.into_inner());
        let has_changed = match *last {
            Some(ref prev) => prev.as_str() != current_cph,
            None => true,
        };
        if has_changed {
            *last = Some(current_cph.to_string());
            "SERIAL_RX"
        } else {
            "DEBUG"
        }
    } else if line == "pong" {
        "TRACE"
    } else {
        "SERIAL_RX"
    }
}

/// Parses an incoming UART frame, updates [`AtmegaRawState`] and [`UsbLinkState`],
/// handles low-level driver sequences (handshake ping, TIC detection), and broadcasts
/// the updated [`DriverSnapshot`] on the `watch` channel.
pub fn parse_incoming_line(line: &str, driver: &SharedDriver) {
    let side = driver.side();
    let rx_category = determine_rx_log_category(line);
    let level = match rx_category {
        "TRACE" => logger::LogLevel::Trace,
        "DEBUG" => logger::LogLevel::Debug,
        _ => logger::LogLevel::Info,
    };
    logger::log_with_level_side("SERIAL_RX", Some(side), level, line);

    let event = parse_line(line);

    match &event {
        ProtocolEvent::Ping => {
            if !driver.sniffer_mode() {
                logger::log("SERIAL", "🤝 Hardware ping detected, sending RaspberryPiModeOK");
                let _ = driver.serial_tx.try_send(Command::RaspberryPiModeOk);
            } else {
                logger::log(
                    "SERIAL",
                    "👂 Hardware ping detected, ignored in Sniffer mode (no RaspberryPiModeOK sent)",
                );
            }
        }
        ProtocolEvent::Side(val) => {
            let side_num = AtmegaRawState::decode_side_number(val);
            logger::debug(
                "SERIAL",
                &format!(
                    "ℹ️ Active socket side = {} (raw: {})",
                    side_num
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "Unknown".to_string()),
                    val
                ),
            );
            if let Some(s) = side_num {
                driver.side.store(s, Ordering::Relaxed);
            }
        }
        ProtocolEvent::StateChange(state_val) => {
            if state_val == "V" {
                logger::log(
                    "SYS",
                    "!!! CRITICAL ALERT: POWER OUTAGE DETECTED (State:V) - IMMINENT SHUTDOWN !!!",
                );
            }
            let prev_state = driver.get_raw_state().state.unwrap_or_default();
            let was_charging = matches!(prev_state.as_str(), "D" | "E" | "G" | "H");
            let is_charging = matches!(state_val.as_str(), "D" | "E" | "G" | "H");
            if is_charging != was_charging {
                reset_serial_filters();
                if was_charging && !is_charging {
                    logger::log(
                        "SERIAL",
                        &format!(
                            "🔌 Charge interrupted/ended (State {}): current set to 0",
                            state_val
                        ),
                    );
                }
            }
        }
        ProtocolEvent::ErrorChange(e) => {
            if e == "0012" {
                logger::log(
                    "SYS",
                    "!!! CRITICAL ALERT: UNDER-VOLTAGE FAULT (E:0012) - IMMINENT SHUTDOWN !!!",
                );
            }
        }
        ProtocolEvent::FmMode(fm_val) => {
            if let Some(mode) = FunctioningMode::from_code(fm_val) {
                logger::debug("SERIAL", &format!("ℹ️ FM Mode detected = {:?}", mode));
            } else {
                logger::debug("SERIAL", &format!("⚠️ Unknown FM Mode detected = {}", fm_val));
            }
        }
        ProtocolEvent::ExternalSignal(enabled) | ProtocolEvent::ExternalSignalForced(enabled) => {
            logger::debug(
                "SERIAL",
                &format!(
                    "ℹ️ FM2 (External Signal) = {}",
                    if *enabled {
                        "1 (BLOCK CHARGE)"
                    } else {
                        "0 (ALLOW CHARGE)"
                    }
                ),
            );
        }
        ProtocolEvent::Derogation(enabled) => {
            logger::debug(
                "SERIAL",
                &format!(
                    "ℹ️ Front button derogation = {}",
                    if *enabled { "ON" } else { "OFF" }
                ),
            );
        }
        ProtocolEvent::SbfState(v) => {
            logger::debug("SERIAL", &format!("ℹ️ SBF (Front Stop Button) = {}", v));
        }
        ProtocolEvent::SessionTicket(ticket) => {
            logger::debug(
                "SERIAL",
                &format!("🎫 Legrand session ticket received: {}", ticket),
            );
        }
        ProtocolEvent::TicTestBaud(val) => {
            let mut should_stop_test = false;
            if let Ok(mut tic) = driver.tic_detection.lock() {
                if tic.is_active {
                    if val != "0" {
                        should_stop_test = true;
                        tic.is_active = false;
                    } else {
                        tic.zero_count += 1;
                        if tic.zero_count >= TICTM_MAX_ZEROS {
                            should_stop_test = true;
                            tic.is_active = false;
                        }
                    }
                }
            }

            if val != "0" {
                logger::log("SERIAL", &format!("🔌 TIC detected: {} baud", val));
                driver.update_raw(|raw| {
                    raw.tic_test_b = Some(val.clone());
                    if should_stop_test {
                        raw.tic_tm = Some(false);
                    }
                });
            } else if should_stop_test {
                logger::log("SERIAL", "🔌 TIC not detected (absent)");
                driver.update_raw(|raw| {
                    raw.tic_test_b = Some("0".to_string());
                    raw.tic_tm = Some(false);
                });
            } else {
                driver.update_raw(|raw| {
                    raw.tic_test_b = Some("0".to_string());
                });
            }

            if should_stop_test {
                logger::debug("SERIAL", "End of automatic TIC detection, sending TICTM:0");
                let _ = driver.serial_tx.try_send(Command::SetTicTestMode(false));
            }
            return;
        }
        _ => {}
    }

    let applied = driver.update_raw(|raw| raw.apply_event(&event));

    if !applied {
        match &event {
            ProtocolEvent::Ping | ProtocolEvent::Pong | ProtocolEvent::TicTestInit => {}
            ProtocolEvent::CommandNotUnderstood(cmd) => {
                logger::log_with_level_side(
                    "SERIAL",
                    Some(side),
                    logger::LogLevel::Warn,
                    &format!(
                        "⚠️ Command rejected by charging station (Default): '{}'",
                        cmd
                    ),
                );
            }
            ProtocolEvent::Unknown(raw_text) => {
                logger::log_with_level_side(
                    "SERIAL",
                    Some(side),
                    logger::LogLevel::Warn,
                    &format!(
                        "⚠️ Unknown serial frame received (unrecognized protocol): '{}'",
                        raw_text
                    ),
                );
            }
            _ => {
                logger::log_with_level_side(
                    "SERIAL",
                    Some(side),
                    logger::LogLevel::Warn,
                    &format!(
                        "⚠️ Unmapped serial frame received (not retained in raw state model): '{}'",
                        line
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_bounded_line_reading_normal() {
        let data = b"State:A\r\nI:0.0 V:230.0\n";
        let mut reader = Cursor::new(data);
        let mut byte_buf = Vec::new();

        let remaining = (MAX_SERIAL_LINE_LEN - byte_buf.len()) as u64;
        let n = (&mut reader)
            .take(remaining)
            .read_until(b'\n', &mut byte_buf)
            .unwrap();
        assert!(n > 0);
        assert!(byte_buf.ends_with(b"\n"));
        let clean = String::from_utf8_lossy(&byte_buf).trim().to_string();
        assert_eq!(clean, "State:A");
        byte_buf.clear();

        let remaining = (MAX_SERIAL_LINE_LEN - byte_buf.len()) as u64;
        let n = (&mut reader)
            .take(remaining)
            .read_until(b'\n', &mut byte_buf)
            .unwrap();
        assert!(n > 0);
        assert!(byte_buf.ends_with(b"\n"));
        let clean = String::from_utf8_lossy(&byte_buf).trim().to_string();
        assert_eq!(clean, "I:0.0 V:230.0");
    }

    #[test]
    fn test_bounded_line_reading_noise_overflow() {
        let noise = vec![b'X'; 300];
        let mut reader = Cursor::new(noise);
        let mut byte_buf = Vec::new();

        let remaining = (MAX_SERIAL_LINE_LEN - byte_buf.len()) as u64;
        let n = (&mut reader)
            .take(remaining)
            .read_until(b'\n', &mut byte_buf)
            .unwrap();

        assert_eq!(n, MAX_SERIAL_LINE_LEN);
        assert_eq!(byte_buf.len(), MAX_SERIAL_LINE_LEN);
        assert!(!byte_buf.ends_with(b"\n"));

        if byte_buf.len() >= MAX_SERIAL_LINE_LEN {
            byte_buf.clear();
        }
        assert_eq!(byte_buf.len(), 0);
    }

    #[test]
    fn test_determine_rx_log_category_cc_cph_cci_filtering() {
        reset_serial_filters();
        assert_eq!(determine_rx_log_category("CC:24"), "SERIAL_RX");
        assert_eq!(determine_rx_log_category("CC:24"), "TRACE");
        assert_eq!(determine_rx_log_category("CC:23"), "SERIAL_RX");

        assert_eq!(determine_rx_log_category("CPh:Mono"), "SERIAL_RX");
        assert_eq!(determine_rx_log_category("CPh:Mono"), "DEBUG");
        assert_eq!(determine_rx_log_category("CPh:Tri"), "SERIAL_RX");

        assert_eq!(determine_rx_log_category("CCI:16.00"), "SERIAL_RX");
        assert_eq!(determine_rx_log_category("CCI:16.20"), "TRACE");
        assert_eq!(determine_rx_log_category("CCI:16.55"), "TRACE");
        assert_eq!(determine_rx_log_category("CCI:16.85"), "SERIAL_RX");
        assert_eq!(determine_rx_log_category("pong"), "TRACE");
    }

    #[tokio::test]
    async fn test_start_tic_detection_state_a_guard() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(10);
        let driver = Arc::new(DriverHandle::new(tx));

        // 1. Unknown state -> NotInStateA
        assert_eq!(
            start_tic_detection(&driver).await,
            Err(TicStartError::NotInStateA("Unknown".to_string()))
        );

        // 2. State B -> NotInStateA
        driver.update_raw(|r| r.state = Some("B".to_string()));
        assert_eq!(
            start_tic_detection(&driver).await,
            Err(TicStartError::NotInStateA("B".to_string()))
        );

        // 3. State A -> Ok
        driver.update_raw(|r| r.state = Some("A".to_string()));
        assert_eq!(start_tic_detection(&driver).await, Ok(()));
        assert_eq!(driver.get_raw_state().tic_tm, Some(true));
        assert_eq!(rx.recv().await, Some(Command::SetTicTestMode(true)));
    }

    #[tokio::test]
    async fn test_parse_incoming_line_logs_warning_on_unmapped_and_unknown() {
        let (tx, _rx) = tokio::sync::mpsc::channel(10);
        let driver = Arc::new(DriverHandle::new(tx));

        // 1. Mapped frame should update state and not produce unmapped warning
        parse_incoming_line("State:B", &driver);
        assert_eq!(driver.get_raw_state().state.as_deref(), Some("B"));

        // 2. Unmapped frame (e.g. EM:12345, RFIDId:ABCDEF) should log a warning
        parse_incoming_line("EM:12345", &driver);
        let recent = logger::get_recent_logs();
        assert!(recent.iter().any(|entry| {
            entry.level == "WARN"
                && entry.message.contains("Unmapped serial frame")
                && entry.message.contains("EM:12345")
        }));

        // 3. Completely unknown frame should log an unknown frame warning
        parse_incoming_line("UNKNOWN_CUSTOM_CMD:999", &driver);
        let recent = logger::get_recent_logs();
        assert!(recent.iter().any(|entry| {
            entry.level == "WARN"
                && entry.message.contains("Unknown serial frame")
                && entry.message.contains("UNKNOWN_CUSTOM_CMD:999")
        }));

        // 4. Default command rejection should log Default warning
        parse_incoming_line("Default:BadCmd", &driver);
        let recent = logger::get_recent_logs();
        assert!(recent.iter().any(|entry| {
            entry.level == "WARN"
                && entry.message.contains("Default")
                && entry.message.contains("BadCmd")
        }));

        // 5. TICTestC:XXX must be silently ignored (no warning emitted)
        parse_incoming_line("TICTestC:1200", &driver);
        let recent = logger::get_recent_logs();
        assert!(!recent.iter().any(|entry| {
            entry.level == "WARN" && entry.message.contains("TICTestC:1200")
        }));

        // 6. FM2F and SBF update FM2 and SB in raw state without warning
        parse_incoming_line("FM2F:1", &driver);
        parse_incoming_line("SBF:0", &driver);
        assert_eq!(driver.get_raw_state().fm2, Some(true));
        assert_eq!(driver.get_raw_state().sb, Some(false));
    }
}
