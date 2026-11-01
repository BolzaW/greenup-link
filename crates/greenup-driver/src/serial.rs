use crate::logger;
use crate::state::SharedState;
use greenup_protocol::{commands::FunctioningMode, Command, parser::{parse_line, ProtocolEvent}};
use std::io::{BufRead, BufReader, Read, Write};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::telemetry::SerialStatus;

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
            port: std::env::var("GREENUP_SERIAL_PORT")
                .unwrap_or_else(|_| "/dev/ttyUSB0".to_string()),
            baud_rate: 115_200,
            timeout_ms: 100,
            watchdog_timeout_secs: 6,
        }
    }
}

/// Maximum allowed length in bytes for a single line from the serial port (256 bytes).
/// Protects against OOM caused by electrical noise on the UART bus streaming bytes without delimiters.
pub const MAX_SERIAL_LINE_LEN: usize = 256;

const TICTM_MAX_ZEROS: u32 = 5;


pub async fn trigger_init_sequence(state: &SharedState) {
    let _ = state.serial_tx.send(Command::RaspberryPiModeOk).await;
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    for cmd in Command::startup_queries() {
        let _ = state.serial_tx.send(cmd).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

pub enum TicStartError {
    /// The charging station is not in `State:A` (contains the current Legrand state).
    NotInStateA(String),
    /// The channel to the serial port is closed.
    Serial,
}

/// Starts TIC detection (`TICTM:1`), ONLY if the Legrand state is `A`.
///
/// Centralized safeguard: if the charging station is not in `State:A`, the function does
/// absolutely nothing (no frame sent, no state modified). The driver then takes care
/// of sending `TICTM:0` at the end of the detection.
pub async fn start_tic_detection(state: &SharedState) -> Result<(), TicStartError> {
    let current_state = match state.telemetry.lock() {
        Ok(tel) => tel.greenup_state.clone(),
        Err(_) => String::from("Unknown"),
    };

    if current_state != "A" {
        logger::log("SERIAL", &format!("⛔ TIC detection ignored: charging station not free (State: {})", current_state));
        return Err(TicStartError::NotInStateA(current_state));
    }

    if let Ok(mut tic) = state.tic_detection.lock() { tic.is_active = true; tic.zero_count = 0; }
    if let Ok(mut tel) = state.telemetry.lock() { tel.tic_mode = "detecting".to_string(); }
    state
        .serial_tx
        .send(Command::SetTicTestMode(true))
        .await
        .map_err(|_| TicStartError::Serial)
}


pub fn send_init_sequence_sync(port: &mut Box<dyn serialport::SerialPort>, _state: &SharedState) {
    let hello = Command::RaspberryPiModeOk;
    let _ = port.write_all(hello.encode().as_bytes());
    logger::log("SERIAL_TX", &hello.as_frame());
    std::thread::sleep(Duration::from_millis(200));

    for cmd in Command::startup_queries() {
        let _ = port.write_all(cmd.encode().as_bytes());
        logger::log("SERIAL_TX", &cmd.as_frame());
        std::thread::sleep(Duration::from_millis(150));
    }


}

pub fn run_serial_loop(state: SharedState, rx_channel: tokio::sync::mpsc::Receiver<Command>) {
    run_serial_loop_with_options(state, rx_channel, SerialOptions::default());
}

pub fn run_serial_loop_with_options(
    state: SharedState,
    mut rx_channel: tokio::sync::mpsc::Receiver<Command>,
    options: SerialOptions,
) {
    let port_name = options.port;
    let baud_rate = options.baud_rate;
    let watchdog_timeout = Duration::from_secs(options.watchdog_timeout_secs);
    let mut last_logged_disconnect: Option<Instant> = None;

    loop {
        // 1. Attempt to open serial port without crashing
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

                if let Ok(mut tel) = state.telemetry.lock() {
                    tel.serial.status = SerialStatus::UsbDisconnected;
                    tel.serial.port = port_name.clone();
                    tel.serial.message = format!("USB cable not detected ({})", e);
                    tel.serial.last_rx_secs_ago = None;
                    tel.greenup_state = "DISCONNECTED".to_string();
                    tel.iec_state = None;
                    tel.voltage = 0.0;
                    tel.current = 0.0;
                    tel.power = 0.0;
                }

                while let Ok(cmd) = rx_channel.try_recv() {
                    logger::debug("SERIAL", &format!("🗑️ Command {} dropped (USB disconnected)", cmd.as_frame()));
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

        if let Ok(mut tel) = state.telemetry.lock() {
            tel.serial.status = SerialStatus::UsbConnecting;
            tel.serial.port = port_name.clone();
            tel.serial.message = "USB port connected, waiting for response from station board".to_string();
            tel.serial.last_rx_secs_ago = None;
        }

        let clone_port = match port.try_clone() {
            Ok(p) => p,
            Err(e) => {
                logger::error("SERIAL", &format!("❌ Failed to clone serial port {}: {}", port_name, e));
                std::thread::sleep(Duration::from_secs(2));
                continue;
            }
        };

        let is_running = Arc::new(AtomicBool::new(true));
        let last_rx_time = Arc::new(std::sync::Mutex::new(None::<Instant>));
        let (internal_tx, internal_rx) = std::sync::mpsc::channel::<String>();

        let is_running_reader = is_running.clone();
        let last_rx_reader = last_rx_time.clone();
        let state_rx = state.clone();
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

                                if let Ok(mut tel) = state_rx.telemetry.lock() {
                                    if tel.serial.status != SerialStatus::UsbConnected {
                                        tel.serial.status = SerialStatus::UsbConnected;
                                        tel.serial.message = "USB serial link active - Station connected".to_string();
                                        logger::info(
                                            "SERIAL",
                                            &format!("✅ USB serial communication established on {} (ATmega link active)", port_name_reader),
                                        );
                                    }
                                    tel.serial.last_rx_secs_ago = Some(0);
                                }

                                let _ = internal_tx.send(clean_line.to_string());
                                parse_incoming_line(clean_line, &state_rx);
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
                            &format!("🔌 I/O error on USB serial port {} : {}. Cable disconnected or link broken.", port_name_reader, e),
                        );
                        is_running_reader.store(false, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });

        reset_serial_filters();
        send_init_sequence_sync(&mut port, &state);

        let session_start = Instant::now();
        let mut last_watchdog_probe = Instant::now();
        let mut last_watchdog_warn = Instant::now();

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
                let encoded = cmd.encode();
                let expects_response = cmd.expects_response();
                let mut retries = 1;
                let mut success = false;

                while retries >= 0 && !success && is_running.load(Ordering::Relaxed) {
                    while let Ok(_) = internal_rx.try_recv() {}

                    logger::log("SERIAL_TX", encoded.trim());
                    if let Err(e) = port.write_all(encoded.as_bytes()) {
                        logger::error("SERIAL", &format!("❌ Serial write error on {} : {}", port_name, e));
                        is_running.store(false, Ordering::Relaxed);
                        break;
                    }

                    if expects_response {
                        let start_wait = Instant::now();
                        let mut matched = false;
                        let mut got_default = false;

                        while start_wait.elapsed() < Duration::from_millis(1000) && is_running.load(Ordering::Relaxed) {
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
                                logger::warn("SERIAL", &format!("❌ Command {} rejected by station (Default), retry: {}", cmd.as_frame(), retries));
                            } else {
                                logger::warn("SERIAL", &format!("⏳ Command {} timeout, retry: {}", cmd.as_frame(), retries));
                            }
                            retries -= 1;
                        }
                    } else {
                        success = true;
                    }
                }
            } else {
                std::thread::sleep(Duration::from_millis(50));
            }

            let (last_rx_opt, current_status) = {
                let rx_lock = last_rx_time.lock().unwrap_or_else(|e| e.into_inner());
                let tel_lock = state.telemetry.lock().unwrap_or_else(|e| e.into_inner());
                (*rx_lock, tel_lock.serial.status.clone())
            };

            let elapsed_since_rx = match last_rx_opt {
                Some(t) => t.elapsed(),
                None => session_start.elapsed(),
            };

            if let Ok(mut tel) = state.telemetry.lock() {
                tel.serial.last_rx_secs_ago = Some(elapsed_since_rx.as_secs());
            }

            if elapsed_since_rx > watchdog_timeout {
                if current_status == SerialStatus::UsbConnected {
                    if let Ok(mut tel) = state.telemetry.lock() {
                        tel.serial.status = SerialStatus::UsbNoResponse;
                        tel.serial.message = "No response from station board (station powered off or ATmega frozen)".to_string();
                    }
                    if last_watchdog_warn.elapsed() >= Duration::from_secs(10) {
                        logger::warn(
                            "SERIAL",
                            &format!(
                                "⚠️ No response from station board on {} for {}s. Check station power or power-cycle station if ATmega is frozen.",
                                port_name,
                                elapsed_since_rx.as_secs()
                            ),
                        );
                        last_watchdog_warn = Instant::now();
                    }
                } else if current_status == SerialStatus::UsbConnecting && elapsed_since_rx > Duration::from_secs(5) {
                    if let Ok(mut tel) = state.telemetry.lock() {
                        tel.serial.status = SerialStatus::UsbNoResponse;
                        tel.serial.message = "No response from station board (station powered off or ATmega frozen)".to_string();
                    }
                    if last_watchdog_warn.elapsed() >= Duration::from_secs(10) {
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

                if last_watchdog_probe.elapsed() >= Duration::from_secs(5) {
                    last_watchdog_probe = Instant::now();
                    let hello = Command::RaspberryPiModeOk;
                    if let Err(e) = port.write_all(hello.encode().as_bytes()) {
                        logger::error("SERIAL", &format!("❌ Write error on {} during probe: {}", port_name, e));
                        is_running.store(false, Ordering::Relaxed);
                    }
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

// Function that analyzes each line coming from the board and updates the shared memory

use crate::telemetry::Telemetry;

fn update_iec_state(tel: &mut Telemetry) {
    if tel.greenup_state == "V" {
        tel.iec_state = Some("F".to_string());
    } else if tel.greenup_state == "R" || tel.greenup_state == "X" {
        tel.iec_state = Some("E".to_string());
    } else if let Some(false) = tel.t2c_enabled {
        match tel.cp_voltage {
            Some(12) => tel.iec_state = Some("A".to_string()),
            Some(9) => tel.iec_state = Some("B".to_string()),
            Some(6) => tel.iec_state = Some("C".to_string()),
            _ => {}
        }
    } else {
        match tel.greenup_state.as_str() {
            "A" | "L" => tel.iec_state = Some("A".to_string()),
            "B" | "C" | "I" | "W" | "M" => tel.iec_state = Some("B".to_string()),
            "D" | "E" => tel.iec_state = Some("C".to_string()),
            _ => {}
        }
    }
}

pub fn update_charge_status(tel: &mut Telemetry) {
    // Authorized to charge if FM2 is OFF (false) or D is ON (true)
    let is_blocked_by_fm2 = tel.fm2_state.unwrap_or(false);
    let is_forced_by_d = tel.d_state.unwrap_or(false);
    
    // Also T2C must be authorized (default to true if unknown to avoid blocking)
    let is_t2c_authorized = tel.t2c_enabled.unwrap_or(true);

    tel.charge_authorized = is_t2c_authorized && (!is_blocked_by_fm2 || is_forced_by_d);
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
/// - `CCI:xx`: logged in INFO (`"SERIAL_RX"`) only if variation >= 1.0A (otherwise `"TRACE"`)
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
                None => 1.0, // First measurement -> log in INFO
            };
            if diff >= 1.0 {
                *last = Some(current_val);
                "SERIAL_RX" // Logged as INFO
            } else {
                "TRACE" // Variations < 1A logged as TRACE
            }
        } else {
            "TRACE"
        }
    } else if let Some(val_str) = line.strip_prefix("CC:") {
        if let Ok(current_val) = val_str.trim().parse::<u32>() {
            let mut last = LAST_LOGGED_CC.lock().unwrap_or_else(|e| e.into_inner());
            let diff = match *last {
                Some(prev) => (current_val as i32 - prev as i32).abs(),
                None => 1, // First measurement -> log in INFO
            };
            if diff >= 1 {
                *last = Some(current_val);
                "SERIAL_RX" // Logged as INFO on >= 1A change
            } else {
                "TRACE" // Identical CC values logged as TRACE
            }
        } else {
            "TRACE"
        }
    } else if let Some(val_str) = line.strip_prefix("CPh:").or_else(|| line.strip_prefix("Cph:")) {
        let current_cph = val_str.trim();
        let mut last = LAST_LOGGED_CPH.lock().unwrap_or_else(|e| e.into_inner());
        let has_changed = match *last {
            Some(ref prev) => prev.as_str() != current_cph,
            None => true, // First reception -> log in INFO
        };
        if has_changed {
            *last = Some(current_cph.to_string());
            "SERIAL_RX" // Logged as INFO on first or changed
        } else {
            "DEBUG" // Unchanged repetitive CPh announcements logged as DEBUG
        }
    } else {
        "SERIAL_RX"
    }
}

fn parse_incoming_line(line: &str, state: &SharedState) {
    let rx_category = determine_rx_log_category(line);
    logger::log(rx_category, line);

    let event = parse_line(line);

    match event {
        ProtocolEvent::Ping => {
            logger::log("SERIAL", "🤝 Hardware ping detected, sending RaspberryPiModeOK");
            let _ = state.serial_tx.try_send(Command::RaspberryPiModeOk);
        }
        ProtocolEvent::SoftwareVersion(v) => {
            if let Ok(mut info) = state.info.lock() { info.software_version = v; }
        }
        ProtocolEvent::HardwareVersion(v) => {
            if let Ok(mut info) = state.info.lock() { info.hardware_version = v; }
        }
        ProtocolEvent::SerialNumber(v) => {
            if let Ok(mut info) = state.info.lock() { info.serial_number = v; }
        }
        ProtocolEvent::Reference(v) => {
            if let Ok(mut info) = state.info.lock() {
                let spec = crate::hardware_specs::HardwareCapabilities::from_reference(&v);
                if !spec.is_known {
                    logger::log("SERIAL", &format!("⚠️ Unknown reference ({}), assuming basic model (Single-phase 4.6kW)", v));
                } else {
                    logger::log("SERIAL", &format!("ℹ️ Model identified: {}", spec.name));
                }
                info.reference = v;
                info.capabilities = Some(spec);
            }
        }
        ProtocolEvent::WeekYearProduction(v) => {
            if let Ok(mut info) = state.info.lock() { info.week_year_production = v; }
        }
        ProtocolEvent::BluetoothState(enabled) => {
            if let Ok(mut info) = state.info.lock() { info.bluetooth_enabled = Some(enabled); }
        }
        ProtocolEvent::MacAddress(val) => {
            if let Ok(mut info) = state.info.lock() { info.mac_address = Some(val); }
        }
        ProtocolEvent::Side(val) => {
            let side_num = match val.as_str() {
                "49" | "1" => Some(1),
                "50" | "2" => Some(2),
                _ => val.parse::<u8>().ok(),
            };
            logger::log("SERIAL", &format!("ℹ️ Active socket side = {} (raw: {})", side_num.map(|s| s.to_string()).unwrap_or_else(|| "Unknown".to_string()), val));
            if let Ok(mut info) = state.info.lock() {
                info.side = side_num;
            }
        }
        ProtocolEvent::Dip1(_) => {}
        ProtocolEvent::ChargePhases(_) => {}
        ProtocolEvent::StateChange(state_val) => {
            if state_val == "V" {
                logger::log("SYS", "!!! CRITICAL ALERT: POWER OUTAGE DETECTED (State:V) - IMMINENT SHUTDOWN !!!");
            }
            if let Ok(mut tel) = state.telemetry.lock() {
                let was_charging = tel.greenup_state == "D" || tel.greenup_state == "E";
                let is_charging = state_val == "D" || state_val == "E";

                if is_charging && !was_charging {
                    // Resuming or starting charge: restart power integration timer
                    tel.last_power_update = Some(std::time::Instant::now());
                    tel.charge_complete = false;
                    reset_serial_filters();
                } else if was_charging && !is_charging {
                    // Contactor opened: instantaneous current and power drop immediately to zero
                    tel.current = 0.0;
                    tel.power = 0.0;
                    tel.last_power_update = None;
                    reset_serial_filters();
                    logger::log("SERIAL", &format!("🔌 Charge interrupted/ended (State {}): current & power set to 0", state_val));
                }

                if state_val == "A" || state_val == "L" {
                    // Vehicle unplugged or station idle: reset all session counters
                    tel.current = 0.0;
                    tel.power = 0.0;
                    tel.energy = 0.0;
                    tel.last_power_update = None;
                    tel.charge_complete = false;
                    reset_serial_filters();
                }

                tel.greenup_state = state_val.clone();
                update_iec_state(&mut tel);
            }
        }
        ProtocolEvent::ErrorChange(e) => {
            if e == "0012" {
                logger::log("SYS", "!!! CRITICAL ALERT: UNDER-VOLTAGE FAULT (E:0012) - IMMINENT SHUTDOWN !!!");
            }
            if let Ok(mut tel) = state.telemetry.lock() { tel.error_code = e; }
        }
        ProtocolEvent::ChargeComplete => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.charge_complete = true; }
        }
        ProtocolEvent::FmMode(fm_val) => {
            if let Some(mode) = FunctioningMode::from_code(&fm_val) {
                logger::log("SERIAL", &format!("ℹ️ FM Mode detected = {:?}", mode));
            } else {
                logger::log("SERIAL", &format!("⚠️ Unknown FM Mode detected = {}", fm_val));
            }
        }
        ProtocolEvent::ExternalSignal(enabled) => {
            logger::log("SERIAL", &format!("ℹ️ FM2 (External Signal) = {}", if enabled { "1 (BLOCK CHARGE)" } else { "0 (ALLOW CHARGE)" }));
            if let Ok(mut tel) = state.telemetry.lock() { tel.fm2_state = Some(enabled); update_charge_status(&mut tel); }
        }
        ProtocolEvent::Derogation(enabled) => {
            logger::log("SERIAL", &format!("ℹ️ Front button derogation = {}", if enabled { "ON" } else { "OFF" }));
            if let Ok(mut tel) = state.telemetry.lock() { tel.d_state = Some(enabled); update_charge_status(&mut tel); }
        }
                ProtocolEvent::TicTestBaud(val) => {
            let mut should_stop_test = false;
            if let Ok(mut tic) = state.tic_detection.lock() {
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
                if let Ok(mut tel) = state.telemetry.lock() { tel.tic_mode = val; }
            } else if should_stop_test {
                logger::log("SERIAL", "🔌 TIC not detected (absent)");
                if let Ok(mut tel) = state.telemetry.lock() { tel.tic_mode = "0".to_string(); }
            }

            if should_stop_test {
                logger::log("SERIAL", "End of automatic TIC detection, sending TICTM:0");
                let _ = state.serial_tx.try_send(Command::SetTicTestMode(false));
            }
        }
        ProtocolEvent::TicTestInit => {}
        ProtocolEvent::Voltage(v) => {
            if let Ok(mut tel) = state.telemetry.lock() {
                tel.voltage = v;
                if tel.greenup_state == "D" || tel.greenup_state == "E" {
                    tel.power = tel.voltage * tel.current;
                } else {
                    tel.power = 0.0;
                }
            }
        }
        ProtocolEvent::Current(c) => {
            if let Ok(mut tel) = state.telemetry.lock() {
                if tel.greenup_state == "D" || tel.greenup_state == "E" {
                    let previous_power = tel.power;
                    tel.current = c;
                    tel.power = tel.voltage * tel.current;
                    let now = std::time::Instant::now();
                    if let Some(last_time) = tel.last_power_update {
                        let elapsed_hours = last_time.elapsed().as_secs_f32() / 3600.0;
                        let energy_wh = ((previous_power + tel.power) / 2.0) * elapsed_hours;
                        tel.energy += energy_wh;
                    }
                    tel.last_power_update = Some(now);
                } else {
                    tel.current = 0.0;
                    tel.power = 0.0;
                    tel.last_power_update = None;
                }
            }
        }
        ProtocolEvent::LimitAmps(limit) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.limit_amps = limit; }
        }
        ProtocolEvent::EliotLimitAmps(limit) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.eliot_limit_amps = Some(limit); }
        }
        ProtocolEvent::CpVoltage(v) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.cp_voltage = Some(v); update_iec_state(&mut tel); }
        }
        ProtocolEvent::T2CEnabled(v) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.t2c_enabled = Some(v); update_iec_state(&mut tel); update_charge_status(&mut tel); }
        }
        ProtocolEvent::SbState(v) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.charge_paused = !v; }
        }
        ProtocolEvent::SbfState(v) => {
            logger::log("SERIAL", &format!("ℹ️ SBF (Front Stop Button) = {}", v));
            if let Ok(mut tel) = state.telemetry.lock() { tel.charge_paused = !v; }
        }
        ProtocolEvent::Energy(e) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.energy = e; }
        }
        ProtocolEvent::Frequency(f) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.frequency = f; }
        }
        ProtocolEvent::CommandNotUnderstood(cmd) => {
            logger::log("SERIAL", &format!("⚠️ Command not recognized by the charging station: {}", cmd));
        }
        ProtocolEvent::Unknown(_) => {}
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

        // First line
        let remaining = (MAX_SERIAL_LINE_LEN - byte_buf.len()) as u64;
        let n = (&mut reader).take(remaining).read_until(b'\n', &mut byte_buf).unwrap();
        assert!(n > 0);
        assert!(byte_buf.ends_with(b"\n"));
        let clean = String::from_utf8_lossy(&byte_buf).trim().to_string();
        assert_eq!(clean, "State:A");
        byte_buf.clear();

        // Second line
        let remaining = (MAX_SERIAL_LINE_LEN - byte_buf.len()) as u64;
        let n = (&mut reader).take(remaining).read_until(b'\n', &mut byte_buf).unwrap();
        assert!(n > 0);
        assert!(byte_buf.ends_with(b"\n"));
        let clean = String::from_utf8_lossy(&byte_buf).trim().to_string();
        assert_eq!(clean, "I:0.0 V:230.0");
    }

    #[test]
    fn test_bounded_line_reading_noise_overflow() {
        // Continuous 300 bytes without newline
        let noise = vec![b'X'; 300];
        let mut reader = Cursor::new(noise);
        let mut byte_buf = Vec::new();

        let remaining = (MAX_SERIAL_LINE_LEN - byte_buf.len()) as u64;
        let n = (&mut reader).take(remaining).read_until(b'\n', &mut byte_buf).unwrap();

        // Must cap at MAX_SERIAL_LINE_LEN (256 bytes) and NOT end with newline
        assert_eq!(n, MAX_SERIAL_LINE_LEN);
        assert_eq!(byte_buf.len(), MAX_SERIAL_LINE_LEN);
        assert!(!byte_buf.ends_with(b"\n"));

        // Overflow condition triggers buffer clear
        if byte_buf.len() >= MAX_SERIAL_LINE_LEN {
            byte_buf.clear();
        }
        assert_eq!(byte_buf.len(), 0);
    }

    #[test]
    fn test_bounded_line_reading_invalid_utf8_glitch() {
        // Frame containing raw binary electrical noise / glitch bytes (0xFF, 0xFE)
        let glitch_frame = vec![0xFF, 0xFE, b'O', b'K', b'\n'];
        let mut reader = Cursor::new(glitch_frame);
        let mut byte_buf = Vec::new();

        let remaining = (MAX_SERIAL_LINE_LEN - byte_buf.len()) as u64;
        let n = (&mut reader).take(remaining).read_until(b'\n', &mut byte_buf).unwrap();
        assert!(n > 0);
        assert!(byte_buf.ends_with(b"\n"));

        // from_utf8_lossy handles glitch bytes gracefully without panic or error
        let clean = String::from_utf8_lossy(&byte_buf).trim().to_string();
        assert!(clean.contains("OK"));
    }

    #[test]
    fn test_determine_rx_log_category_cc_filtering() {
        reset_serial_filters();

        // First CC is logged as INFO (SERIAL_RX)
        assert_eq!(determine_rx_log_category("CC:24"), "SERIAL_RX");
        // Same CC is filtered to TRACE
        assert_eq!(determine_rx_log_category("CC:24"), "TRACE");
        assert_eq!(determine_rx_log_category("CC:24"), "TRACE");

        // Change by 1A is logged as INFO
        assert_eq!(determine_rx_log_category("CC:23"), "SERIAL_RX");
        assert_eq!(determine_rx_log_category("CC:23"), "TRACE");

        // Change by >= 1A (e.g. down to 13A) is logged as INFO
        assert_eq!(determine_rx_log_category("CC:13"), "SERIAL_RX");
        assert_eq!(determine_rx_log_category("CC:13"), "TRACE");

        // CC with leading zero (CC:09)
        assert_eq!(determine_rx_log_category("CC:09"), "SERIAL_RX");
        assert_eq!(determine_rx_log_category("CC:09"), "TRACE");
    }

    #[test]
    fn test_determine_rx_log_category_cph_filtering() {
        reset_serial_filters();

        // First CPh is logged as INFO (SERIAL_RX)
        assert_eq!(determine_rx_log_category("CPh:Mono"), "SERIAL_RX");
        // Repetitive identical CPh is demoted to DEBUG
        assert_eq!(determine_rx_log_category("CPh:Mono"), "DEBUG");
        assert_eq!(determine_rx_log_category("Cph:Mono"), "DEBUG");

        // Change to Tri is logged as INFO
        assert_eq!(determine_rx_log_category("CPh:Tri"), "SERIAL_RX");
        assert_eq!(determine_rx_log_category("CPh:Tri"), "DEBUG");
    }

}
