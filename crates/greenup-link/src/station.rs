//! High-level station supervisor and state interpreter (`AppState` & `StationManager`).
//!
//! Responsibilities:
//! - Subscribing to `greenup-driver`'s [`DriverSnapshot`] via `tokio::sync::watch`.
//! - Translating low-level [`AtmegaRawState`] and [`UsbLinkState`] into high-level [`Telemetry`],
//!   [`BoardInfo`], [`ConnectorData`], and [`StationSnapshot`].
//! - Resolving [`HardwareCapabilities`] from the Legrand commercial reference (`hardware_specs`).
//! - Requesting the discovery and initialization of the 2nd USB port from `greenup-driver`
//!   when a dual-socket reference (`charging_points == 2`) is identified.
//! - Checking hardware TIC eligibility (`has_tic && state == "A"`) and triggering TIC detection on the driver.
//! - Managing power/energy integration and driving [`crate::session`] CSV/TXT recording.

use std::sync::{
    atomic::{AtomicBool, AtomicI64, AtomicU8, Ordering},
    Arc, Mutex, RwLock,
};
use tokio::sync::mpsc;

use crate::hardware_specs::HardwareCapabilities;
pub use crate::telemetry::{
    BoardInfo, ConnectorData, OcppChargePointStatus, SerialConnection, SerialStatus, StationData,
    StationSnapshot, Telemetry,
};
use greenup_driver::{
    logger, DriverHandle, DriverSnapshot, SerialOptions, SharedDriver, TicStartError,
};
use greenup_protocol::{commands::FunctioningMode, Command};

#[derive(Default, Debug)]
struct SyncCursor {
    last_seq: u64,
    last_state: String,
    last_cci_count: u64,
    last_ticket_count: u64,
    last_cph: Option<String>,
    auto_tic_triggered: bool,
    last_cc_el_reset_attempt: Option<std::time::Instant>,
}

/// High-level supervisor state for a single charge point (Side 1 or Side 2).
/// Wraps a low-level [`SharedDriver`] and maintains the interpreted domain model.
pub struct AppState {
    /// Underlying low-level serial/USB driver handle.
    pub driver: SharedDriver,
    /// Charge point side: 1 for Side 1 (Right / Droit), 2 for Side 2 (Left / Gauche).
    pub side: AtomicU8,
    /// Passive UART listen-only mode (`true` = Sniffer mode, zero TX commands).
    pub sniffer_mode: AtomicBool,
    /// Duration (in seconds) to ignore external `disable` commands during `local_override`.
    /// `-1` = locked until end of charge, `0` = disabled, `> 0` = grace period in seconds.
    pub local_override_disable_lock_secs: AtomicI64,
    /// High-level interpreted telemetry (power, energy, IEC/OCPP state, external voltage).
    pub telemetry: Mutex<Telemetry>,
    /// High-level board metadata and hardware capabilities deduced from `Reference:`.
    pub info: Mutex<BoardInfo>,
    /// Command sender to the underlying serial driver thread.
    pub serial_tx: mpsc::Sender<Command>,
    /// Incremental cursor tracking synchronized driver snapshots.
    cursor: Mutex<SyncCursor>,
}

impl AppState {
    pub fn new(serial_tx: mpsc::Sender<Command>) -> Self {
        Self::new_with_side(1, serial_tx)
    }

    pub fn new_with_side(side: u8, serial_tx: mpsc::Sender<Command>) -> Self {
        Self::new_with_side_and_sniffer(side, false, serial_tx)
    }

    pub fn new_with_side_and_sniffer(
        side: u8,
        sniffer_mode: bool,
        serial_tx: mpsc::Sender<Command>,
    ) -> Self {
        let driver = Arc::new(DriverHandle::new_with_side_and_sniffer(
            side,
            sniffer_mode,
            serial_tx,
        ));
        Self::from_driver(driver, env!("CARGO_PKG_VERSION"))
    }

    /// Creates a high-level `AppState` attached to an existing low-level [`SharedDriver`].
    pub fn from_driver(driver: SharedDriver, link_version: &str) -> Self {
        let side = driver.side();
        let sniffer_mode = driver.sniffer_mode();
        let serial_tx = driver.serial_tx.clone();

        let mut tel = Telemetry::default();
        tel.side = side;
        tel.serial.sniffer_mode = sniffer_mode;

        let mut info = BoardInfo::default();
        info.side = Some(side);
        info.link_version = link_version.to_string();

        Self {
            driver,
            side: AtomicU8::new(side),
            sniffer_mode: AtomicBool::new(sniffer_mode),
            local_override_disable_lock_secs: AtomicI64::new(30),
            telemetry: Mutex::new(tel),
            info: Mutex::new(info),
            serial_tx,
            cursor: Mutex::new(SyncCursor::default()),
        }
    }

    /// Spawns a background task that listens to the driver's `tokio::sync::watch` channel
    /// and immediately interprets every low-level state change.
    pub fn start_driver_watcher(self: &Arc<Self>) {
        let state = self.clone();
        let mut watch_rx = self.driver.subscribe();
        tokio::spawn(async move {
            loop {
                if watch_rx.changed().await.is_err() {
                    break;
                }
                let snap = watch_rx.borrow().clone();
                state.apply_driver_snapshot(&snap);
            }
        });
    }

    /// Synchronizes the high-level state with the latest snapshot from `self.driver` if newer.
    pub fn sync_from_driver(&self) {
        let snap = self.driver.get_snapshot();
        self.apply_driver_snapshot(&snap);
    }

    /// Interprets a low-level [`DriverSnapshot`] (`AtmegaRawState` + `UsbLinkState`) into
    /// high-level [`BoardInfo`], [`Telemetry`], power/energy integration, and session events.
    pub fn apply_driver_snapshot(&self, snap: &DriverSnapshot) {
        let mut should_auto_start_tic = false;
        let mut should_query_cau = false;

        {
            let mut cursor = self.cursor.lock().unwrap_or_else(|e| e.into_inner());
            if snap.seq == 0 || snap.seq <= cursor.last_seq {
                return;
            }
            cursor.last_seq = snap.seq;

            let raw = &snap.atmega;
            let usb = &snap.usb;

            // 1. Update side if reported by ATmega
            if let Some(s) = raw.side {
                self.side.store(s, Ordering::Relaxed);
            }
            let side = self.side();

            // 2. Update BoardInfo & HardwareCapabilities from Reference
            {
                let mut info = self.info.lock().unwrap_or_else(|e| e.into_inner());
                info.side = Some(side);
                if let Some(ref v) = raw.software_version {
                    info.software_version = v.clone();
                }
                if let Some(ref v) = raw.hardware_version {
                    info.hardware_version = v.clone();
                }
                if let Some(ref v) = raw.serial_number {
                    info.serial_number = v.clone();
                }
                if let Some(ref v) = raw.week_year_production {
                    info.week_year_production = v.clone();
                }
                if let Some(bt) = raw.bt {
                    info.bluetooth_enabled = Some(bt);
                }
                if let Some(ref mac) = raw.mac_adr {
                    info.mac_address = Some(mac.clone());
                }
                if let Some(ref r) = raw.reference {
                    if info.reference != *r {
                        let spec = HardwareCapabilities::from_reference(r);
                        if !spec.is_known {
                            logger::log(
                                "SERIAL",
                                &format!(
                                    "⚠️ Unknown reference ({}), assuming basic model (Single-phase 4.6kW)",
                                    r
                                ),
                            );
                        } else {
                            logger::log("SERIAL", &format!("ℹ️ Model identified: {}", spec.name));
                        }
                        info.reference = r.clone();
                        info.capabilities = Some(spec);
                    }
                }
                info.board_code = compute_board_code(
                    &info.week_year_production,
                    &info.software_version,
                    &info.serial_number,
                    &info.hardware_version,
                    &info.reference,
                );
            }

            // 3. Update Telemetry, State transitions, Power/Energy, and Sessions
            let mut should_clear_raw_em_derogation = false;
            let mut should_clear_raw_fcs = false;
            let mut should_reset_cc_to_max = false;
            {
                let mut tel = self.telemetry.lock().unwrap_or_else(|e| e.into_inner());
                tel.side = side;
                tel.serial.sniffer_mode = usb.sniffer_mode;
                self.sniffer_mode.store(usb.sniffer_mode, Ordering::Relaxed);

                let prev_usb_status = tel.serial.status.clone();
                tel.serial.status = usb.status.clone();
                tel.serial.port = usb.port.clone();
                tel.serial.message = usb.message.clone();
                tel.serial.last_rx_secs_ago = usb.last_rx_secs_ago;

                if prev_usb_status != SerialStatus::UsbConnected
                    && usb.status == SerialStatus::UsbConnected
                    && tel.voltage == 0.0
                {
                    tel.voltage = 230.0;
                }

                // Check 60s State:A grace period timeout
                if crate::session::check_state_a_timeout_for_side(side, &tel) {
                    tel.energy = 0.0;
                    tel.current = 0.0;
                    tel.power = 0.0;
                    tel.last_power_update = None;
                    tel.charge_complete = false;
                }

                if let Some(cc) = raw.cc {
                    tel.limit_amps = cc;
                    if tel.desired_limit_amps.is_none() {
                        tel.desired_limit_amps = Some(cc);
                    }
                }
                if let Some(cc_el) = raw.cc_el {
                    tel.eliot_limit_amps = Some(cc_el);
                }
                if let Some(cp) = raw.cp {
                    tel.cp_voltage = Some(cp);
                }
                if let Some(t2c) = raw.t2c {
                    tel.t2c_enabled = Some(t2c);
                }
                if let Some(dom_2pc) = raw.dom_2pc {
                    tel.dom_2pc_enabled = Some(dom_2pc);
                }
                if let Some(t2f) = raw.t2f {
                    tel.t2f_forced = Some(t2f);
                }
                if let Some(dom_2pf) = raw.dom_2pf {
                    tel.dom_2pf_forced = Some(dom_2pf);
                }
                if let Some(cau) = raw.cau {
                    tel.cau_authorized = Some(cau);
                }
                if let Some(fm2) = raw.fm2 {
                    tel.fm2_state = Some(fm2);
                }
                if let Some(d) = raw.d {
                    tel.d_state = Some(d);
                }
                if let Some(ref em) = raw.em {
                    tel.em_mask = Some(em.clone());
                }
                if let Some(fcs) = raw.fcs {
                    tel.charge_complete = fcs;
                }
                if let Some(ref err) = raw.error {
                    tel.error_code = err.clone();
                }
                if raw.tic_tm == Some(true) {
                    tel.tic_mode = "detecting".to_string();
                } else if let Some(ref baud) = raw.tic_test_b {
                    tel.tic_mode = baud.clone();
                }
                tel.tic_active = matches!(tel.tic_mode.as_str(), "1200" | "9600");

                if let Some(ref phase) = raw.cph {
                    if cursor.last_cph.as_ref() != Some(phase) {
                        cursor.last_cph = Some(phase.clone());
                        crate::session::update_phase_for_side(side, phase);
                    }
                }

                // State machine transition handling
                if let Some(ref state_val) = raw.state {
                    if *state_val != cursor.last_state {
                        let was_charging =
                            matches!(tel.greenup_state.as_str(), "D" | "E" | "G" | "H");
                        let is_charging = matches!(state_val.as_str(), "D" | "E" | "G" | "H");
                        tel.greenup_state = state_val.clone();
                        cursor.last_state = state_val.clone();

                        if is_charging && !was_charging {
                            tel.last_power_update = Some(std::time::Instant::now());
                            tel.charge_complete = false;
                            should_clear_raw_fcs = true;
                        } else if was_charging && !is_charging {
                            tel.current = 0.0;
                            tel.power = 0.0;
                            tel.last_power_update = None;
                        }

                        if state_val == "DISCONNECTED" {
                            if crate::session::is_active_for_side(side) {
                                crate::session::end_session_for_side(
                                    side,
                                    "Driver shutdown / Serial disconnected",
                                    &tel,
                                );
                            }
                            tel.iec_state = None;
                            tel.voltage = 0.0;
                            tel.current = 0.0;
                            tel.power = 0.0;
                            tel.last_power_update = None;
                            tel.charge_complete = false;
                            should_clear_raw_fcs = true;
                            tel.em_mask = Some(greenup_driver::raw_state::clear_em_derogation_bit(
                                tel.em_mask.as_deref(),
                            ));
                            tel.local_override = false;
                            tel.local_override_started_at = None;
                            should_clear_raw_em_derogation = true;
                        } else if crate::session::is_immediate_termination_state(state_val) {
                            if crate::session::is_active_for_side(side) {
                                let reason = match state_val.as_str() {
                                    "L" => "Cable unlocked / unlatched (State: L)".to_string(),
                                    _ => format!("Safety / fault termination (State: {})", state_val),
                                };
                                crate::session::end_session_for_side(side, &reason, &tel);
                            }
                            tel.current = 0.0;
                            tel.power = 0.0;
                            tel.energy = 0.0;
                            tel.last_power_update = None;
                            tel.charge_complete = false;
                            should_clear_raw_fcs = true;
                            tel.em_mask = Some(greenup_driver::raw_state::clear_em_derogation_bit(
                                tel.em_mask.as_deref(),
                            ));
                            tel.local_override = false;
                            tel.local_override_started_at = None;
                            should_clear_raw_em_derogation = true;
                            should_reset_cc_to_max = true;
                        } else if state_val == "A" {
                            tel.current = 0.0;
                            tel.power = 0.0;
                            tel.last_power_update = None;
                            tel.em_mask = Some(greenup_driver::raw_state::clear_em_derogation_bit(
                                tel.em_mask.as_deref(),
                            ));
                            tel.local_override = false;
                            tel.local_override_started_at = None;
                            should_clear_raw_em_derogation = true;
                            should_reset_cc_to_max = true;

                            if crate::session::is_active_for_side(side) {
                                crate::session::on_state_change_for_side(side, state_val, &tel);
                            } else {
                                tel.energy = 0.0;
                                tel.charge_complete = false;
                                should_clear_raw_fcs = true;
                            }
                        } else if crate::session::is_connected_state(state_val) {
                            if crate::session::is_active_for_side(side) {
                                crate::session::on_state_change_for_side(side, state_val, &tel);
                            } else if crate::session::is_session_start_state(state_val) {
                                crate::session::start_session_for_side(side, state_val, &tel);
                            }
                        }
                    }
                }

                tel.update_iec_state();
                tel.update_charge_status();

                // Instantaneous phase current update & trapezoidal energy integration
                if raw.cci_updates_count > cursor.last_cci_count {
                    cursor.last_cci_count = raw.cci_updates_count;
                    let l1 = raw.cci.unwrap_or(0.0);
                    let l2 = raw.cci_l2.unwrap_or(0.0);
                    let l3 = raw.cci_l3.unwrap_or(0.0);
                    tel.evaluate_voltage();
                    if matches!(tel.greenup_state.as_str(), "D" | "E" | "G" | "H") {
                        let previous_power = tel.power;
                        tel.current = l1;
                        tel.power = tel.voltage * (l1 + l2 + l3);
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
                    crate::session::on_telemetry_tick_for_side(side, &tel);
                }

                if raw.wt_updates_count > cursor.last_ticket_count {
                    cursor.last_ticket_count = raw.wt_updates_count;
                    if let Some(ref ticket) = raw.wt {
                        crate::session::record_ticket_for_side(side, ticket);
                    }
                }
            }

            if should_clear_raw_em_derogation || (should_clear_raw_fcs && raw.fcs.is_some()) {
                let cleared_em = greenup_driver::raw_state::clear_em_derogation_bit(raw.em.as_deref());
                let update_em = should_clear_raw_em_derogation && raw.em.as_deref() != Some(cleared_em.as_str());
                let update_fcs = should_clear_raw_fcs && raw.fcs.is_some();
                if update_em || update_fcs {
                    self.driver.update_raw(|r| {
                        if update_em {
                            r.em = Some(cleared_em);
                        }
                        if update_fcs {
                            r.fcs = None;
                        }
                    });
                }
            }

            // Reset RAM-only direct current limit (CC:32) on idle / end-of-session
            if should_reset_cc_to_max
                && !usb.sniffer_mode
                && usb.status == SerialStatus::UsbConnected
                && raw.cc.is_some_and(|v| v != 32)
            {
                logger::debug(
                    "SERIAL",
                    &format!(
                        "⚡ Side {}: Idle / end-of-session state reached with CC={}A → resetting CC:32",
                        side,
                        raw.cc.unwrap_or(0)
                    ),
                );
                if self.serial_tx.try_send(Command::SetDirectCurrentLimit(32)).is_ok() {
                    if let Ok(mut tel) = self.telemetry.lock() {
                        tel.desired_limit_amps = Some(32);
                    }
                }
            }

            // Ensure EEPROM-backed CCEl stays non-limiting at 32A (retry with 5s cooldown until confirmed)
            if raw.cc_el == Some(32) {
                cursor.last_cc_el_reset_attempt = None;
            } else if !usb.sniffer_mode
                && usb.status == SerialStatus::UsbConnected
                && usb.init_completed
                && raw.cc_el.is_some_and(|v| v != 32)
            {
                let should_retry = cursor
                    .last_cc_el_reset_attempt
                    .is_none_or(|t| t.elapsed() >= std::time::Duration::from_secs(5));
                if should_retry {
                    logger::debug(
                        "SERIAL",
                        &format!(
                            "🛡️ Side {}: CCEl={}A != 32A → enforcing CCEl:32 (non-limiting EEPROM baseline)",
                            side,
                            raw.cc_el.unwrap_or(0)
                        ),
                    );
                    if self.serial_tx.try_send(Command::SetCurrentLimit(32)).is_ok() {
                        cursor.last_cc_el_reset_attempt = Some(std::time::Instant::now());
                    }
                }
            }

            // 4. High-level automatic TIC detection & safe CAU? check after driver init completes
            if !usb.init_completed {
                cursor.auto_tic_triggered = false;
            } else if usb.init_completed
                && !cursor.auto_tic_triggered
                && !usb.sniffer_mode
            {
                cursor.auto_tic_triggered = true;
                should_auto_start_tic = true;
                let is_active_session_state = matches!(
                    raw.state.as_deref(),
                    Some("C") | Some("D") | Some("E") | Some("F") | Some("G") | Some("H") | Some("I")
                );
                let tic_detected = raw
                    .tic_test_b
                    .as_deref()
                    .is_some_and(|b| !b.is_empty() && b != "0");
                if raw.cau.is_none() && !is_active_session_state && !tic_detected {
                    should_query_cau = true;
                }
            }
        }

        if should_query_cau {
            logger::debug(
                "SERIAL",
                "🔍 Querying CAU? after init sequence (no active charge in progress)",
            );
            let _ = self.serial_tx.try_send(Command::GetChargeAuthorization);
        }

        if should_auto_start_tic {
            match check_tic_eligibility(self) {
                Ok(()) => {
                    logger::debug(
                        "SERIAL",
                        "🚀 Starting automatic TIC detection after init sequence (Hardware compatible & State:A)",
                    );
                    if let Ok(mut tel) = self.telemetry.lock() {
                        tel.tic_mode = "detecting".to_string();
                    }
                    let _ = self
                        .driver
                        .try_send_command(Command::SetTicTestMode(true));
                    if let Ok(mut tic) = self.driver.tic_detection.lock() {
                        tic.is_active = true;
                        tic.zero_count = 0;
                    }
                    self.driver.update_raw(|raw| {
                        raw.tic_tm = Some(true);
                    });
                }
                Err(TicStartError::NotSupported) => {
                    logger::log(
                        "SERIAL",
                        "ℹ️ Skipping automatic TIC detection: hardware model does not support TIC",
                    );
                }
                Err(TicStartError::NotInStateA(ref current_state)) => {
                    logger::log(
                        "SERIAL",
                        &format!(
                            "ℹ️ Skipping automatic TIC detection: charging station not free (State: {})",
                            current_state
                        ),
                    );
                }
                Err(_) => {}
            }
        }
    }

    /// Returns current active side number (1 or 2).
    pub fn side(&self) -> u8 {
        self.side.load(Ordering::Relaxed)
    }

    /// Sets the side number once discovered or reconfigured.
    pub fn set_side(&self, s: u8) {
        self.side.store(s, Ordering::Relaxed);
        self.driver.set_side(s);
    }

    /// Records the desired current setpoint (from TX `CC:XX`).
    pub fn set_desired_limit_amps(&self, amps: u32) {
        if let Ok(mut tel) = self.telemetry.lock() {
            tel.desired_limit_amps = Some(amps);
        }
    }

    /// Returns `true` if the station is operating in Sniffer mode (passive listen-only, zero TX).
    pub fn sniffer_mode(&self) -> bool {
        self.sniffer_mode.load(Ordering::Relaxed)
    }

    /// Returns `true` if an active TIC interface has been detected (baudrate `"1200"` or `"9600"`).
    pub fn is_tic_active(&self) -> bool {
        self.sync_from_driver();
        if let Ok(tel) = self.telemetry.lock() {
            if matches!(tel.tic_mode.as_str(), "1200" | "9600") {
                return true;
            }
        }
        self.driver
            .get_snapshot()
            .atmega
            .tic_test_b
            .as_deref()
            .is_some_and(|b| !b.is_empty() && b != "0")
    }

    /// Configures the `local_override` disable lock duration in seconds (`-1`, `0`, or `> 0`).
    pub fn set_local_override_disable_lock_secs(&self, secs: i64) {
        self.local_override_disable_lock_secs
            .store(secs, Ordering::Relaxed);
    }

    /// Checks whether an external `disable` command should be ignored due to `local_override`.
    /// Returns `Some(-1)` if locked for the whole charge, `Some(remaining_secs)` if within the
    /// grace period, or `None` if `disable` is allowed.
    pub fn is_disable_locked_by_local_override(&self) -> Option<i64> {
        self.sync_from_driver();
        let lock_secs = self.local_override_disable_lock_secs.load(Ordering::Relaxed);
        if lock_secs == 0 {
            return None;
        }
        let tel = self.telemetry.lock().unwrap_or_else(|e| e.into_inner());
        if !tel.local_override {
            return None;
        }
        if lock_secs < 0 {
            return Some(-1);
        }
        if let Some(started_at) = tel.local_override_started_at {
            let elapsed = started_at.elapsed().as_secs() as i64;
            if elapsed < lock_secs {
                return Some(lock_secs - elapsed);
            }
        }
        None
    }

    /// Clears `local_override` when an authorized software `disable` is executed.
    pub fn clear_local_override(&self) {
        let current_em = self.driver.get_raw_state().em;
        let cleared_em = greenup_driver::raw_state::clear_em_derogation_bit(current_em.as_deref());
        if current_em.as_deref() != Some(cleared_em.as_str()) {
            self.driver.update_raw(|raw| {
                raw.em = Some(cleared_em);
            });
        }
        if let Ok(mut tel) = self.telemetry.lock() {
            tel.em_mask = Some(greenup_driver::raw_state::clear_em_derogation_bit(
                tel.em_mask.as_deref(),
            ));
            tel.local_override = false;
            tel.local_override_started_at = None;
            tel.update_charge_status();
        }
    }

    /// Returns true if the USB serial link is active and actively communicating with the ATmega.
    pub fn is_serial_connected(&self) -> bool {
        self.sync_from_driver();
        self.telemetry
            .lock()
            .map(|t| t.serial.status == SerialStatus::UsbConnected)
            .unwrap_or(false)
    }

    /// Converts the current charge point state into a standardized ConnectorData representation.
    pub fn to_connector_data(&self) -> ConnectorData {
        self.sync_from_driver();
        let info = self.info.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let tel = {
            let mut t = self.telemetry.lock().unwrap_or_else(|e| e.into_inner());
            t.evaluate_voltage();
            t.clone()
        };
        tel.to_connector_data(&info)
    }
}

pub type SharedState = Arc<AppState>;

/// High-level check verifying whether the charging station is eligible for TIC detection:
/// 1. The hardware reference in `HardwareCapabilities` must physically support TIC (`c.is_known && c.has_tic`).
/// 2. The Legrand operating state must be `State:A` (Available / Free).
pub fn check_tic_eligibility(state: & AppState) -> Result<(), TicStartError> {
    state.sync_from_driver();
    let has_tic = match state.info.lock() {
        Ok(info) => info
            .capabilities
            .as_ref()
            .map(|c| c.is_known && c.has_tic)
            .unwrap_or(false),
        Err(_) => false,
    };

    if !has_tic {
        return Err(TicStartError::NotSupported);
    }

    let current_state = match state.telemetry.lock() {
        Ok(tel) => tel.greenup_state.clone(),
        Err(_) => String::from("Unknown"),
    };

    if current_state != "A" {
        return Err(TicStartError::NotInStateA(current_state));
    }

    Ok(())
}

/// High-level orchestrator starting TIC detection (`TICTM:1`) on the low-level driver
/// ONLY if the station's [`HardwareCapabilities`] supports TIC and the station is in `State:A`.
pub async fn start_tic_detection(state: &SharedState) -> Result<(), TicStartError> {
    match check_tic_eligibility(state) {
        Ok(()) => {}
        Err(TicStartError::NotSupported) => {
            logger::log(
                "SERIAL",
                "⛔ TIC detection rejected: charging station hardware does not support TIC",
            );
            return Err(TicStartError::NotSupported);
        }
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

    if let Ok(mut tel) = state.telemetry.lock() {
        tel.tic_mode = "detecting".to_string();
    }
    state.driver.update_raw(|raw| {
        if raw.state.is_none() {
            raw.state = Some("A".to_string());
        }
    });

    greenup_driver::start_tic_detection(&state.driver).await
}

/// High-level initialization sequence: runs the low-level driver handshake + startup queries,
/// safely queries `CAU?` when no active charge is in progress, and initiates TIC detection
/// if the station hardware model supports TIC and is in `State:A`.
pub async fn trigger_init_sequence(state: &SharedState) {
    greenup_driver::trigger_init_sequence(&state.driver).await;
    if !state.sniffer_mode() {
        let is_charging = state
            .telemetry
            .lock()
            .map(|t| matches!(t.greenup_state.as_str(), "D" | "E" | "G" | "H"))
            .unwrap_or(false);
        if !is_charging {
            let _ = state.serial_tx.send(Command::GetChargeAuthorization).await;
        }
    }
    let _ = start_tic_detection(state).await;
}

/// Supported charge and actuator control actions exposed via REST (`POST /api/charge`)
/// and MQTT (`.../control/charge/set`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeAction {
    /// Universal supervisor enable (`FM2:0` + `DOK`).
    Enable,
    /// Universal supervisor disable (`DNOK` + `FM2:1`), respecting `local_override`.
    Disable,
    /// Return to standalone / direct charge behavior: conditionally clears all locks and overrides without modifying `CC`.
    Standalone,
    /// Authorize / resume charging (`CAUOK`). Keeps cable locked and session open.
    Authorize,
    /// Cleanly suspend charging (`CAUNOK` -> `State:W` -> `State:B`/`J`/`F`). Keeps cable locked and session open.
    Suspend,
    /// Simulate physical Start button press (`SBOK`). Re-arms session from `State:M`/`P`.
    StartSession,
    /// Simulate physical Stop button press (`SBNOK`). Closes session (`WT/CT`) and unlocks T2S cable (`State:M`/`P`).
    StopSession,
    /// Enable the Type 2 socket (`T2COK`).
    EnableType2,
    /// Disable the Type 2 socket (`T2CNOK`). Aborts/unlocks any T2S session and freezes T2S detection in `State:A`.
    DisableType2,
    /// Enable the auxiliary domestic Schuko (2P+T) socket (`2PCOK`).
    EnableDomestic,
    /// Disable the auxiliary domestic Schuko (2P+T) socket (`2PCNOK`).
    DisableDomestic,
    /// Force immediate charging on Type 2 socket (`T2FOK`), bypassing schedules, `CAU`, and contact conditioning.
    ForceType2,
    /// Release forced Type 2 charging override (`T2FNOK`).
    UnforceType2,
    /// Force immediate charging on domestic Schuko socket (`2PFOK`), bypassing schedules, `CAU`, and contact conditioning.
    ForceDomestic,
    /// Release forced domestic charging override (`2PFNOK`).
    UnforceDomestic,
    /// Activate off-peak / schedule derogation (`DOK`) without modifying `FM2`.
    EnableDerogation,
    /// Deactivate off-peak / schedule derogation (`DNOK`) without modifying `FM2`.
    DisableDerogation,
    /// Stop any active charge (`State:W`), close session, and unlock the Type 2 cable (`Unlock`).
    Unlock,
    /// Put the station into persistent deep sleep (`SOK` -> `State:Y`, OCPP `Unavailable`).
    Sleep,
    /// Wake the station from deep sleep (`SNOK` -> `State:A`).
    Wake,
}

impl ChargeAction {
    /// Parses a REST API action string.
    pub fn from_api_str(action: &str) -> Option<Self> {
        match action.trim() {
            "enable" => Some(Self::Enable),
            "disable" => Some(Self::Disable),
            "standalone" | "autonomous" => Some(Self::Standalone),
            "authorize" => Some(Self::Authorize),
            "suspend" => Some(Self::Suspend),
            "start_session" => Some(Self::StartSession),
            "stop_session" => Some(Self::StopSession),
            "enable_type2" => Some(Self::EnableType2),
            "disable_type2" => Some(Self::DisableType2),
            "enable_domestic" => Some(Self::EnableDomestic),
            "disable_domestic" => Some(Self::DisableDomestic),
            "force_type2" => Some(Self::ForceType2),
            "unforce_type2" => Some(Self::UnforceType2),
            "force_domestic" => Some(Self::ForceDomestic),
            "unforce_domestic" => Some(Self::UnforceDomestic),
            "enable_derogation" => Some(Self::EnableDerogation),
            "disable_derogation" => Some(Self::DisableDerogation),
            "unlock" => Some(Self::Unlock),
            "sleep" => Some(Self::Sleep),
            "wake" => Some(Self::Wake),
            _ => None,
        }
    }

    /// Parses an MQTT payload for `.../control/charge/set` (includes Home Assistant switch ON/OFF aliases).
    pub fn from_mqtt_str(payload: &str) -> Option<Self> {
        let lower = payload.trim().to_lowercase();
        match lower.as_str() {
            "on" | "1" | "true" => Some(Self::Enable),
            "off" | "0" | "false" => Some(Self::Disable),
            other => Self::from_api_str(other),
        }
    }

    /// Returns the human-readable description and exact ATmega command sequence for this action.
    pub fn commands(self) -> (&'static str, Vec<Command>) {
        match self {
            Self::Enable => (
                "⚡ Charge enable (FM2:0 + DOK)",
                vec![Command::SetExternalSignal(false), Command::SetDerogation(true)],
            ),
            Self::Disable => (
                "🛑 Charge disable (DNOK + FM2:1)",
                vec![Command::SetDerogation(false), Command::SetExternalSignal(true)],
            ),
            Self::Standalone => (
                "🔓 Standalone charge (clear locks & overrides)",
                vec![],
            ),
            Self::Authorize => (
                "▶ Charge authorize (CAUOK)",
                vec![Command::SetChargeAuthorization(true)],
            ),
            Self::Suspend => (
                "⏸ Charge suspend (CAUNOK)",
                vec![Command::SetChargeAuthorization(false)],
            ),
            Self::StartSession => (
                "▶ Start session / button press (SBOK)",
                vec![Command::SetStartButton(true)],
            ),
            Self::StopSession => (
                "⏹ Stop session & unlock / button press (SBNOK)",
                vec![Command::SetStartButton(false)],
            ),
            Self::EnableType2 => (
                "🔌 Enable Type 2 socket (T2COK)",
                vec![Command::AuthorizeType2(true)],
            ),
            Self::DisableType2 => (
                "🚫 Disable Type 2 socket (T2CNOK)",
                vec![Command::AuthorizeType2(false)],
            ),
            Self::EnableDomestic => (
                "🔌 Enable domestic Schuko socket (2PCOK)",
                vec![Command::AuthorizeDomestic(true)],
            ),
            Self::DisableDomestic => (
                "🚫 Disable domestic Schuko socket (2PCNOK)",
                vec![Command::AuthorizeDomestic(false)],
            ),
            Self::ForceType2 => (
                "⚡ Force Type 2 charge (T2FOK)",
                vec![Command::ForceType2(true)],
            ),
            Self::UnforceType2 => (
                "🔓 Unforce Type 2 charge (T2FNOK)",
                vec![Command::ForceType2(false)],
            ),
            Self::ForceDomestic => (
                "⚡ Force domestic Schuko charge (2PFOK)",
                vec![Command::ForceDomestic(true)],
            ),
            Self::UnforceDomestic => (
                "🔓 Unforce domestic Schuko charge (2PFNOK)",
                vec![Command::ForceDomestic(false)],
            ),
            Self::EnableDerogation => (
                "🔓 Enable off-peak derogation (DOK)",
                vec![Command::SetDerogation(true)],
            ),
            Self::DisableDerogation => (
                "🔒 Disable off-peak derogation (DNOK)",
                vec![Command::SetDerogation(false)],
            ),
            Self::Unlock => (
                "🔓 Unlock Type 2 cable (Unlock)",
                vec![Command::Unlock],
            ),
            Self::Sleep => (
                "💤 Enter station deep sleep (SOK -> State:Y)",
                vec![Command::SetSleep(true)],
            ),
            Self::Wake => (
                "☀ Wake station from deep sleep (SNOK -> State:A)",
                vec![Command::SetSleep(false)],
            ),
        }
    }
}

/// Outcome of executing a [`ChargeAction`] on a charge point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeActionOutcome {
    Executed,
    IgnoredLocalOverride { remaining_secs: i64 },
    NotSupportedWhileTicActive,
    SerialError,
}

/// Executes a [`ChargeAction`] on the given [`AppState`], enforcing `local_override` protection on `Disable`
/// and rejecting `CAU` actions (`Suspend` / `Authorize`) while an active TIC is detected.
pub async fn execute_charge_action(
    point: &AppState,
    action: ChargeAction,
    log_source: &str,
) -> ChargeActionOutcome {
    if matches!(action, ChargeAction::Suspend | ChargeAction::Authorize) && point.is_tic_active() {
        logger::warn(
            log_source,
            &format!(
                "⛔ Side {}: Charge action '{:?}' rejected (not supported while TIC active)",
                point.side(),
                action
            ),
        );
        return ChargeActionOutcome::NotSupportedWhileTicActive;
    }

    if action == ChargeAction::Disable {
        if let Some(remaining) = point.is_disable_locked_by_local_override() {
            let lock_desc = if remaining < 0 {
                "locked until end of charge".to_string()
            } else {
                format!("{}s remaining", remaining)
            };
            logger::log(
                log_source,
                &format!(
                    "🛡️ Side {}: Charge disable ignored (local_override active: {})",
                    point.side(),
                    lock_desc
                ),
            );
            return ChargeActionOutcome::IgnoredLocalOverride {
                remaining_secs: remaining,
            };
        }
        point.clear_local_override();
    }

    let (desc, mut cmds) = action.commands();
    let raw_atmega = point.driver.get_snapshot().atmega;
    match action {
        ChargeAction::Enable => {
            if raw_atmega.t2c == Some(false) {
                cmds.push(Command::AuthorizeType2(true));
            }
            if raw_atmega.dom_2pc == Some(false) {
                cmds.push(Command::AuthorizeDomestic(true));
            }
            if !point.is_tic_active() && raw_atmega.cau == Some(false) {
                cmds.push(Command::SetChargeAuthorization(true));
            }
        }
        ChargeAction::Disable => {
            if raw_atmega.t2f == Some(true) {
                cmds.push(Command::ForceType2(false));
            }
            if raw_atmega.dom_2pf == Some(true) {
                cmds.push(Command::ForceDomestic(false));
            }
        }
        ChargeAction::Standalone => {
            let had_local_override = point
                .telemetry
                .lock()
                .map(|t| t.local_override)
                .unwrap_or(false);
            point.clear_local_override();

            // Only issue FM:1 if trapped in Planning/Modbus/OCPP (avoids EEPROM write & CC reset in FM:1/2)
            let prev_cc = raw_atmega.cc;
            let exited_special_fm = matches!(
                raw_atmega.fm.as_deref(),
                Some("4") | Some("5") | Some("6")
            );
            if exited_special_fm {
                cmds.push(Command::SetFunctioningMode(FunctioningMode::DirectCharge));
            }

            if raw_atmega.fm2 != Some(false) {
                cmds.push(Command::SetExternalSignal(false));
            }
            if raw_atmega.d != Some(false) || had_local_override {
                cmds.push(Command::SetDerogation(false));
            }
            if raw_atmega.t2f == Some(true) {
                cmds.push(Command::ForceType2(false));
            }
            if raw_atmega.dom_2pf == Some(true) {
                cmds.push(Command::ForceDomestic(false));
            }
            if raw_atmega.t2c == Some(false) {
                cmds.push(Command::AuthorizeType2(true));
            }
            if raw_atmega.dom_2pc == Some(false) {
                cmds.push(Command::AuthorizeDomestic(true));
            }
            if !point.is_tic_active() && raw_atmega.cau == Some(false) {
                cmds.push(Command::SetChargeAuthorization(true));
            }
            if raw_atmega.sb == Some(false)
                || matches!(raw_atmega.state.as_deref(), Some("M") | Some("P"))
            {
                cmds.push(Command::SetStartButton(true));
            }
            if raw_atmega.state.as_deref() == Some("Y") {
                cmds.push(Command::SetSleep(false));
            }
            if raw_atmega.pa == Some(true) {
                cmds.push(Command::SetPlanningAuthorization(false));
            }
            if raw_atmega.pafc == Some(true) {
                cmds.push(Command::SetPlanningAutoForceCharge(false));
            }
            if raw_atmega.dec.is_some_and(|d| d > 0) {
                cmds.push(Command::SetDeferredStartDelay(0));
            }

            // Preserve CC setpoint if FM:1 had to be issued (since FM:1 resets CC to CCS internally)
            if exited_special_fm {
                if let Some(cc_val) = prev_cc {
                    if cc_val > 0 && cc_val < 32 {
                        cmds.push(Command::SetDirectCurrentLimit(cc_val as u8));
                    }
                }
            }
        }
        _ => {}
    }

    logger::log(
        log_source,
        &format!("Point {}: {}", point.side(), desc),
    );
    for cmd in cmds {
        if point.serial_tx.send(cmd).await.is_err() {
            return ChargeActionOutcome::SerialError;
        }
    }
    ChargeActionOutcome::Executed
}

/// Manages multi-point charging stations (1 or 2 charge points).
pub struct StationManager {
    pub point1: SharedState,
    pub point2: RwLock<Option<SharedState>>,
    pub update_manager: Arc<crate::update::UpdateManager>,
}

impl StationManager {
    pub fn new(point1: SharedState, point2: Option<SharedState>) -> Self {
        let link_version = point1
            .info
            .lock()
            .map(|i| i.link_version.clone())
            .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
        Self {
            point1,
            point2: RwLock::new(point2),
            update_manager: Arc::new(crate::update::UpdateManager::new(&link_version)),
        }
    }

    pub fn update_manager(&self) -> Arc<crate::update::UpdateManager> {
        self.update_manager.clone()
    }

    /// Attaches the second charge point dynamically after startup.
    pub fn attach_point2(&self, p2: SharedState) {
        let lock_secs = self
            .point1
            .local_override_disable_lock_secs
            .load(Ordering::Relaxed);
        p2.set_local_override_disable_lock_secs(lock_secs);
        let mut guard = self.point2.write().unwrap_or_else(|e| e.into_inner());
        *guard = Some(p2);
    }

    /// Configures `local_override_disable_lock_secs` on all active charge points.
    pub fn set_local_override_disable_lock_secs(&self, secs: i64) {
        self.point1.set_local_override_disable_lock_secs(secs);
        let guard = self.point2.read().unwrap_or_else(|e| e.into_inner());
        if let Some(ref p2) = *guard {
            p2.set_local_override_disable_lock_secs(secs);
        }
    }

    /// Watches Point 1's low-level driver via `tokio::sync::watch` and automatically requests
    /// `greenup-driver` to discover and initialize the 2nd USB port if the Legrand reference
    /// corresponds to a Dual Charge Point (`charging_points == 2`).
    pub fn watch_auto_dual_port(
        self: &Arc<Self>,
        configured_port1: Option<String>,
        configured_port2: Option<String>,
        watchdog_timeout_secs: u64,
        link_version: String,
    ) {
        let station = self.clone();
        let mut rx = self.point1.driver.subscribe();

        tokio::spawn(async move {
            loop {
                let ref_opt = rx.borrow().atmega.reference.clone();
                if let Some(ref_str) = ref_opt {
                    if !ref_str.trim().is_empty() {
                        if station.is_dual() {
                            return;
                        }
                        let spec = HardwareCapabilities::from_reference(&ref_str);
                        if spec.charging_points == 2 {
                            let (_p1, port2_target) = greenup_driver::discover_candidate_ports(
                                configured_port1.as_deref(),
                                configured_port2.as_deref(),
                            );
                            logger::log(
                                "SYS",
                                &format!(
                                    "⚡ Station reference '{}' identified as Dual Charge Point: requesting driver to initialize Point 2 on {}",
                                    ref_str, port2_target
                                ),
                            );
                            let sniffer_mode = station.point1.sniffer_mode();
                            let driver2 = greenup_driver::spawn_port(
                                2,
                                sniffer_mode,
                                SerialOptions {
                                    port: port2_target,
                                    baud_rate: greenup_driver::ATMEGA_BAUD_RATE,
                                    timeout_ms: 100,
                                    watchdog_timeout_secs,
                                },
                            );
                            let app_state_2 = Arc::new(AppState::from_driver(driver2, &link_version));
                            app_state_2.set_local_override_disable_lock_secs(
                                station.point1.local_override_disable_lock_secs.load(Ordering::Relaxed),
                            );
                            app_state_2.start_driver_watcher();
                            station.attach_point2(app_state_2);
                            logger::set_dual_station(true);
                        } else {
                            logger::log(
                                "SYS",
                                &format!(
                                    "ℹ️ Station reference '{}' confirmed Single Charge Point",
                                    ref_str
                                ),
                            );
                        }
                        return;
                    }
                }

                if rx.changed().await.is_err() {
                    return;
                }
            }
        });
    }

    /// Returns true if two charging points are configured / active.
    pub fn is_dual(&self) -> bool {
        self.point2
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
    }

    /// Returns the list of active point numbers: `[1]` for single-point, `[1, 2]` for dual-point.
    pub fn available_points(&self) -> Vec<u8> {
        if self.is_dual() {
            vec![1, 2]
        } else {
            vec![1]
        }
    }

    /// Resolves the state for the given side (`1` or `2`).
    pub fn get_point(&self, requested_side: Option<u8>) -> SharedState {
        self.point1.sync_from_driver();
        let guard = self.point2.read().unwrap_or_else(|e| e.into_inner());
        if let Some(ref p2) = *guard {
            p2.sync_from_driver();
        }

        let target = requested_side.unwrap_or(1);
        if self.point1.side() == target {
            return self.point1.clone();
        }
        if let Some(ref p2) = *guard {
            if p2.side() == target {
                return p2.clone();
            }
        }
        if target == 2 {
            guard
                .as_ref()
                .cloned()
                .unwrap_or_else(|| self.point1.clone())
        } else {
            self.point1.clone()
        }
    }

    /// Returns whether the low-level hardware initialization sequence has completed on Point 1.
    pub fn is_init_completed(&self) -> bool {
        self.point1.driver.get_snapshot().usb.init_completed
    }

    /// Returns the current USB connection status of Point 1.
    pub fn usb_status(&self) -> greenup_driver::SerialStatus {
        self.point1.driver.get_snapshot().usb.status
    }

    /// Returns the raw low-level JSON snapshot (`AtmegaRawState` + `UsbLinkState`) for all active USB ports.
    pub fn raw_driver_snapshot_json(&self) -> serde_json::Value {
        let p1_snap = self.point1.driver.get_snapshot();
        let guard = self.point2.read().unwrap_or_else(|e| e.into_inner());
        let mut points = vec![p1_snap];
        if let Some(ref p2) = *guard {
            points.push(p2.driver.get_snapshot());
        }
        serde_json::json!({
            "is_dual": self.is_dual(),
            "ports": points,
        })
    }

    /// Produces a complete, unified `StationSnapshot` aggregating station-level
    /// metadata and all active charge point connectors (OCPP & EVCC aligned).
    pub fn snapshot(&self) -> StationSnapshot {
        self.point1.sync_from_driver();
        let p1_info = self
            .point1
            .info
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let p1_tel = {
            let mut tel = self
                .point1
                .telemetry
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            tel.evaluate_voltage();
            tel.clone()
        };

        let mut connectors = vec![p1_tel.to_connector_data(&p1_info)];

        let is_dual = self.is_dual();
        let available_connectors = self.available_points();

        let guard = self.point2.read().unwrap_or_else(|e| e.into_inner());
        if let Some(ref p2) = *guard {
            p2.sync_from_driver();
            let p2_info = p2.info.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let p2_tel = {
                let mut tel = p2
                    .telemetry
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                tel.evaluate_voltage();
                tel.clone()
            };
            connectors.push(p2_tel.to_connector_data(&p2_info));
        }

        let station = StationData {
            vendor: "Legrand".to_string(),
            model: p1_info.reference.clone(),
            serial_number: p1_info.serial_number.clone(),
            hardware_version: p1_info.hardware_version.clone(),
            firmware_version: p1_info.software_version.clone(),
            link_version: p1_info.link_version.clone(),
            serial_connection: p1_tel.serial.clone(),
            is_dual,
            available_connectors,
            production_date: if p1_info.week_year_production.is_empty() {
                None
            } else {
                Some(p1_info.week_year_production.clone())
            },
            bluetooth_enabled: p1_info.bluetooth_enabled,
            mac_address: p1_info.mac_address.clone(),
            capabilities: p1_info.capabilities.clone(),
            board_code: p1_info.board_code.clone(),
            update: self.update_manager.snapshot(),
        };

        StationSnapshot {
            station,
            connectors,
        }
    }

    /// Produces `StationData` for a specific charge point side (`1` or `2`).
    pub fn station_for_side(&self, requested_side: Option<u8>) -> StationData {
        let point = self.get_point(requested_side);
        let info = point.info.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let tel = point
            .telemetry
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        StationData {
            vendor: "Legrand".to_string(),
            model: info.reference.clone(),
            serial_number: info.serial_number.clone(),
            hardware_version: info.hardware_version.clone(),
            firmware_version: info.software_version.clone(),
            link_version: info.link_version.clone(),
            serial_connection: tel.serial,
            is_dual: self.is_dual(),
            available_connectors: self.available_points(),
            production_date: if info.week_year_production.is_empty() {
                None
            } else {
                Some(info.week_year_production.clone())
            },
            bluetooth_enabled: info.bluetooth_enabled,
            mac_address: info.mac_address.clone(),
            capabilities: info.capabilities.clone(),
            board_code: info.board_code.clone(),
            update: self.update_manager.snapshot(),
        }
    }
}

fn crc16_modbus(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in data {
        crc ^= b as u16;
        for _ in 0..8 {
            if (crc & 1) != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

fn normalize_version_string(v: &str) -> String {
    let clean: String = v.trim().replace(':', "").replace(' ', "");
    if clean.is_empty() {
        return String::new();
    }
    if clean.starts_with('V') {
        clean
    } else if clean.contains('.') {
        format!("V{}", clean)
    } else if clean.len() == 6 {
        format!("V{}.{}.{}", &clean[0..2], &clean[2..4], &clean[4..6])
    } else {
        format!("V{}", clean)
    }
}

/// Computes the board identification CRC16 checksum from the hardware identity registers.
pub fn compute_board_code(
    week_year_production: &str,
    software_version: &str,
    serial_number: &str,
    hardware_version: &str,
    reference: &str,
) -> Option<String> {
    let prod = week_year_production.trim();
    let sw = normalize_version_string(software_version);
    let sn = serial_number.trim();
    let hw = normalize_version_string(hardware_version);
    let r = reference.trim();

    if !prod.is_ascii() || !sw.is_ascii() || !sn.is_ascii() || !hw.is_ascii() || !r.is_ascii() {
        return None;
    }

    let p = prod.as_bytes();
    let s = sw.as_bytes();
    let n = sn.as_bytes();
    let h = hw.as_bytes();
    let rf = r.as_bytes();

    if p.len() < 5 || s.len() < 9 || n.len() < 6 || h.len() < 9 || rf.len() < 6 {
        return None;
    }

    let buf = [
        p[0], s[8], n[2], h[8], rf[1],
        p[4], s[1], n[1], h[7], rf[3],
        p[1], s[4], n[5], h[1], rf[4],
        p[3], s[2], n[0], h[2], rf[5],
        s[5], n[4], h[4], rf[2], s[7], n[3], h[5], rf[0],
    ];

    Some(crc16_modbus(&buf).to_string())
}

pub type SharedStation = Arc<StationManager>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_single_and_dual_point_station_manager_and_driver_sync() {
        let (tx1, _rx1) = tokio::sync::mpsc::channel(10);
        let (tx2, _rx2) = tokio::sync::mpsc::channel(10);
        let p1 = Arc::new(AppState::new_with_side(1, tx1));
        let p2 = Arc::new(AppState::new_with_side(2, tx2));

        // Simulate low-level driver updates on Point 1
        p1.driver.update_both(|raw, usb| {
            usb.status = SerialStatus::UsbConnected;
            raw.reference = Some("058001".to_string());
            raw.serial_number = Some("SIM01".to_string());
            raw.hardware_version = Some("A".to_string());
            raw.software_version = Some("1.03.25".to_string());
            raw.state = Some("A".to_string());
            raw.cc = Some(20);
            raw.cc_el = Some(16);
        });

        let manager = StationManager::new(p1.clone(), Some(p2.clone()));
        let snap = manager.snapshot();
        assert!(snap.station.is_dual);
        assert_eq!(snap.station.model, "058001");
        assert_eq!(snap.station.serial_number, "SIM01");
        assert!(snap.station.capabilities.as_ref().unwrap().has_tic);
        assert_eq!(snap.connectors[0].status.ocpp, OcppChargePointStatus::Available);
        assert_eq!(snap.connectors[0].control.applied_limit_amps, 20);
        assert_eq!(snap.connectors[0].control.setpoint_amps, 32);
    }

    #[tokio::test]
    async fn test_check_tic_eligibility_and_auto_init() {
        let (tx, mut rx) = mpsc::channel(64);
        let state = Arc::new(AppState::new(tx));

        // 1. Without capabilities -> NotSupported
        assert_eq!(check_tic_eligibility(&state), Err(TicStartError::NotSupported));

        // 2. Non-TIC reference (058030) -> NotSupported
        state.driver.update_raw(|raw| {
            raw.reference = Some("058030".to_string());
            raw.state = Some("A".to_string());
        });
        assert_eq!(check_tic_eligibility(&state), Err(TicStartError::NotSupported));

        // 3. TIC reference (058001) in State B -> NotInStateA
        state.driver.update_raw(|raw| {
            raw.reference = Some("058001".to_string());
            raw.state = Some("B".to_string());
        });
        assert_eq!(
            check_tic_eligibility(&state),
            Err(TicStartError::NotInStateA("B".to_string()))
        );

        // 4. TIC reference (058001) in State A -> Ok
        state.driver.update_raw(|raw| {
            raw.state = Some("A".to_string());
        });
        assert_eq!(check_tic_eligibility(&state), Ok(()));

        trigger_init_sequence(&state).await;
        let mut received = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            received.push(cmd);
        }
        assert_eq!(received.first(), Some(&Command::RaspberryPiModeOk));
        assert_eq!(received.last(), Some(&Command::SetTicTestMode(true)));
    }

    #[tokio::test]
    async fn test_local_override_disable_lock_modes() {
        let (tx, _rx) = mpsc::channel(16);
        let state = Arc::new(AppState::new(tx));

        // Activate local_override via driver raw state (State:D + EM:XX1 + FM2:1)
        state.driver.update_both(|raw, usb| {
            usb.status = SerialStatus::UsbConnected;
            raw.state = Some("D".to_string());
            raw.fm2 = Some(true);
            raw.em = Some("XX1".to_string());
        });

        // 1. Default 30s lock -> locked
        assert!(state.is_disable_locked_by_local_override().is_some());

        // 2. 0s -> never locked
        state.set_local_override_disable_lock_secs(0);
        assert_eq!(state.is_disable_locked_by_local_override(), None);

        // 3. -1 -> locked indefinitely until end of charge
        state.set_local_override_disable_lock_secs(-1);
        assert_eq!(state.is_disable_locked_by_local_override(), Some(-1));

        // 4. Expired 30s grace period -> unlocked
        state.set_local_override_disable_lock_secs(30);
        if let Ok(mut tel) = state.telemetry.lock() {
            tel.local_override_started_at =
                Some(std::time::Instant::now() - std::time::Duration::from_secs(35));
        }
        assert_eq!(state.is_disable_locked_by_local_override(), None);

        // 5. clear_local_override resets flag
        state.clear_local_override();
        assert!(!state.to_connector_data().status.local_override);
    }

    #[tokio::test]
    async fn test_compute_board_code_and_station_for_side() {
        assert_eq!(
            compute_board_code("42W22", "V01.18.10", "015184", "V02.01.01", "058001").as_deref(),
            Some("13862")
        );
        assert_eq!(
            compute_board_code("42W22", "011810", "015184", "02.01.01", "058001").as_deref(),
            Some("13862")
        );
        assert_eq!(
            compute_board_code("", "V01.18.10", "015184", "V02.01.01", "058001"),
            None
        );

        let (tx1, _rx1) = mpsc::channel(8);
        let (tx2, _rx2) = mpsc::channel(8);
        let p1 = Arc::new(AppState::new_with_side(1, tx1));
        let p2 = Arc::new(AppState::new_with_side(2, tx2));

        p1.driver.update_raw(|raw| {
            raw.reference = Some("058001".to_string());
            raw.serial_number = Some("015184".to_string());
            raw.hardware_version = Some("V02.01.01".to_string());
            raw.software_version = Some("V01.18.10".to_string());
            raw.week_year_production = Some("42W22".to_string());
        });
        p2.driver.update_raw(|raw| {
            raw.reference = Some("058001".to_string());
            raw.serial_number = Some("015185".to_string());
            raw.hardware_version = Some("V02.01.01".to_string());
            raw.software_version = Some("V01.18.10".to_string());
            raw.week_year_production = Some("42W22".to_string());
        });

        let manager = StationManager::new(p1, Some(p2));
        let s1 = manager.station_for_side(Some(1));
        let s2 = manager.station_for_side(Some(2));
        assert_eq!(s1.serial_number, "015184");
        assert_eq!(s1.board_code.as_deref(), Some("13862"));
        assert_eq!(s2.serial_number, "015185");
        assert_ne!(s2.board_code, s1.board_code);
        assert!(s2.board_code.is_some());
    }

    #[tokio::test]
    async fn test_cau_mapped_to_paused_and_queried_safely_after_init() {
        let (tx, rx) = mpsc::channel(32);
        let state = Arc::new(AppState::new(tx));

        // 1. State M / P maps to status.session_stopped=true; State B maps to session_stopped=false
        state.driver.update_both(|raw, usb| {
            usb.status = SerialStatus::UsbConnected;
            raw.state = Some("M".to_string());
        });
        let conn_m = state.to_connector_data();
        assert!(conn_m.status.session_stopped);
        assert_eq!(conn_m.status.active_socket, "t2s");

        state.driver.update_raw(|raw| {
            raw.state = Some("B".to_string());
        });
        let conn_b = state.to_connector_data();
        assert!(!conn_b.status.session_stopped);
        assert_eq!(conn_b.status.active_socket, "t2s");

        // 2. When init completes in active session states (C, D, I, etc.) or with active TIC, CAU? must NOT be queried
        for active_state in ["C", "D", "I"] {
            let (tx_chg, mut rx_chg) = mpsc::channel(32);
            let state_chg = Arc::new(AppState::new(tx_chg));
            state_chg.driver.update_both(|raw, usb| {
                usb.status = SerialStatus::UsbConnected;
                usb.init_completed = true;
                raw.state = Some(active_state.to_string());
            });
            state_chg.sync_from_driver();
            assert!(rx_chg.try_recv().is_err(), "CAU? should not be queried in State:{}", active_state);
        }

        let (tx_tic, mut rx_tic) = mpsc::channel(32);
        let state_tic = Arc::new(AppState::new(tx_tic));
        state_tic.driver.update_both(|raw, usb| {
            usb.status = SerialStatus::UsbConnected;
            usb.init_completed = true;
            raw.state = Some("B".to_string());
            raw.tic_test_b = Some("9600".to_string());
        });
        state_tic.sync_from_driver();
        assert!(rx_tic.try_recv().is_err(), "CAU? should not be queried when TIC is active");

        // 3. When init completes while idle (State:B) and cau is None, CAU? IS queried
        let (tx_idle, mut rx_idle) = mpsc::channel(32);
        let state_idle = Arc::new(AppState::new(tx_idle));
        state_idle.driver.update_both(|raw, usb| {
            usb.status = SerialStatus::UsbConnected;
            usb.init_completed = true;
            raw.state = Some("B".to_string());
        });
        state_idle.sync_from_driver();
        assert_eq!(rx_idle.recv().await.unwrap(), Command::GetChargeAuthorization);

        // 4. Legacy pause/resume are not accepted by ChargeAction
        assert_eq!(ChargeAction::from_api_str("pause"), None);
        assert_eq!(ChargeAction::from_api_str("resume"), None);
        assert_eq!(ChargeAction::from_mqtt_str("pause"), None);
        assert_eq!(ChargeAction::from_mqtt_str("resume"), None);
        assert_eq!(ChargeAction::from_mqtt_str("ON"), Some(ChargeAction::Enable));
        assert_eq!(ChargeAction::from_mqtt_str("OFF"), Some(ChargeAction::Disable));
        drop(rx);
    }

    #[tokio::test]
    async fn test_adaptive_enable_disable_actions() {
        let (tx, mut rx) = mpsc::channel(32);
        let state = Arc::new(AppState::new(tx));
        state.driver.update_both(|raw, usb| {
            usb.status = SerialStatus::UsbConnected;
            raw.state = Some("B".to_string());
            raw.t2c = Some(false);
            raw.dom_2pc = Some(false);
            raw.cau = Some(false);
        });

        let outcome = execute_charge_action(&state, ChargeAction::Enable, "TEST").await;
        assert_eq!(outcome, ChargeActionOutcome::Executed);
        assert_eq!(rx.recv().await.unwrap(), Command::SetExternalSignal(false));
        assert_eq!(rx.recv().await.unwrap(), Command::SetDerogation(true));
        assert_eq!(rx.recv().await.unwrap(), Command::AuthorizeType2(true));
        assert_eq!(rx.recv().await.unwrap(), Command::AuthorizeDomestic(true));
        assert_eq!(rx.recv().await.unwrap(), Command::SetChargeAuthorization(true));
        assert!(rx.try_recv().is_err());

        // Disable unforces T2F and 2PF when active
        state.driver.update_raw(|raw| {
            raw.t2f = Some(true);
            raw.dom_2pf = Some(true);
        });
        let outcome = execute_charge_action(&state, ChargeAction::Disable, "TEST").await;
        assert_eq!(outcome, ChargeActionOutcome::Executed);
        assert_eq!(rx.recv().await.unwrap(), Command::SetDerogation(false));
        assert_eq!(rx.recv().await.unwrap(), Command::SetExternalSignal(true));
        assert_eq!(rx.recv().await.unwrap(), Command::ForceType2(false));
        assert_eq!(rx.recv().await.unwrap(), Command::ForceDomestic(false));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_cc_el_32_enforcement_and_cc_idle_reset() {
        let (tx, mut rx) = mpsc::channel(32);
        let state = Arc::new(AppState::new(tx));

        // 1. After init completes with CCEl=16A, CCEl:32 is requested once
        state.driver.update_both(|raw, usb| {
            usb.status = SerialStatus::UsbConnected;
            usb.init_completed = true;
            raw.state = Some("D".to_string());
            raw.cc_el = Some(16);
        });
        state.sync_from_driver();
        assert_eq!(rx.recv().await.unwrap(), Command::SetCurrentLimit(32));
        assert!(rx.try_recv().is_err());

        // 2. Subsequent snapshot while still CCEl=16A within 5s cooldown does NOT spam
        state.driver.update_raw(|raw| {
            raw.cci = Some(10.0);
        });
        state.sync_from_driver();
        assert!(rx.try_recv().is_err());

        // 3. Once CCEl=32A is confirmed, entering State:A with CC=16A resets CC:32 (RAM-only)
        state.driver.update_raw(|raw| {
            raw.cc_el = Some(32);
            raw.cc = Some(16);
            raw.state = Some("A".to_string());
        });
        state.sync_from_driver();
        assert_eq!(rx.recv().await.unwrap(), Command::SetDirectCurrentLimit(32));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_standalone_action_conditional_reset() {
        assert_eq!(ChargeAction::from_api_str("standalone"), Some(ChargeAction::Standalone));
        assert_eq!(ChargeAction::from_mqtt_str("standalone"), Some(ChargeAction::Standalone));

        let (tx, mut rx) = mpsc::channel(32);
        let state = Arc::new(AppState::new(tx));

        // 1. Clean nominal state (FM:1, FM2:0, D:0, T2C:1, 2PC:1, T2F:0, 2PF:0, CAU:1, SB:1) -> sends zero commands
        state.driver.update_both(|raw, usb| {
            usb.status = SerialStatus::UsbConnected;
            raw.state = Some("A".to_string());
            raw.fm = Some("1".to_string());
            raw.fm2 = Some(false);
            raw.d = Some(false);
            raw.t2c = Some(true);
            raw.dom_2pc = Some(true);
            raw.t2f = Some(false);
            raw.dom_2pf = Some(false);
            raw.cau = Some(true);
            raw.sb = Some(true);
            raw.pa = Some(false);
            raw.pafc = Some(false);
            raw.dec = Some(0);
        });
        let outcome = execute_charge_action(&state, ChargeAction::Standalone, "TEST").await;
        assert_eq!(outcome, ChargeActionOutcome::Executed);
        assert!(rx.try_recv().is_err());

        // 2. All locks/overrides active (including FM:4, CAU=0 without TIC, Dec:180) in State:B -> restores everything + preserves CC=16
        state.driver.update_raw(|raw| {
            raw.state = Some("B".to_string());
            raw.fm = Some("4".to_string());
            raw.fm2 = Some(true);
            raw.d = Some(true);
            raw.t2f = Some(true);
            raw.dom_2pf = Some(true);
            raw.t2c = Some(false);
            raw.dom_2pc = Some(false);
            raw.cau = Some(false);
            raw.sb = Some(false);
            raw.pa = Some(true);
            raw.pafc = Some(true);
            raw.dec = Some(180);
            raw.cc = Some(16);
        });
        state.sync_from_driver();
        while rx.try_recv().is_ok() {}

        let outcome = execute_charge_action(&state, ChargeAction::Standalone, "TEST").await;
        assert_eq!(outcome, ChargeActionOutcome::Executed);
        assert_eq!(rx.recv().await.unwrap(), Command::SetFunctioningMode(FunctioningMode::DirectCharge));
        assert_eq!(rx.recv().await.unwrap(), Command::SetExternalSignal(false));
        assert_eq!(rx.recv().await.unwrap(), Command::SetDerogation(false));
        assert_eq!(rx.recv().await.unwrap(), Command::ForceType2(false));
        assert_eq!(rx.recv().await.unwrap(), Command::ForceDomestic(false));
        assert_eq!(rx.recv().await.unwrap(), Command::AuthorizeType2(true));
        assert_eq!(rx.recv().await.unwrap(), Command::AuthorizeDomestic(true));
        assert_eq!(rx.recv().await.unwrap(), Command::SetChargeAuthorization(true));
        assert_eq!(rx.recv().await.unwrap(), Command::SetStartButton(true));
        assert_eq!(rx.recv().await.unwrap(), Command::SetPlanningAuthorization(false));
        assert_eq!(rx.recv().await.unwrap(), Command::SetPlanningAutoForceCharge(false));
        assert_eq!(rx.recv().await.unwrap(), Command::SetDeferredStartDelay(0));
        assert_eq!(rx.recv().await.unwrap(), Command::SetDirectCurrentLimit(16));
        assert!(rx.try_recv().is_err());

        // 3. Sleep State:Y -> sends SNOK
        state.driver.update_raw(|raw| {
            raw.state = Some("Y".to_string());
            raw.fm = Some("1".to_string());
            raw.fm2 = Some(false);
            raw.d = Some(false);
            raw.t2f = Some(false);
            raw.dom_2pf = Some(false);
            raw.t2c = Some(true);
            raw.dom_2pc = Some(true);
            raw.cau = Some(true);
            raw.sb = Some(true);
            raw.pa = Some(false);
            raw.pafc = Some(false);
            raw.dec = Some(0);
        });
        state.sync_from_driver();
        while rx.try_recv().is_ok() {}
        let outcome = execute_charge_action(&state, ChargeAction::Standalone, "TEST").await;
        assert_eq!(outcome, ChargeActionOutcome::Executed);
        assert_eq!(rx.recv().await.unwrap(), Command::SetSleep(false));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_desired_and_applied_limit_amps_lifecycle() {
        let (tx, _rx) = tokio::sync::mpsc::channel(16);
        let state = Arc::new(AppState::new_with_side(1, tx));
        let manager = StationManager::new(state.clone(), None);

        // 1. At startup, desired_limit_amps is None; first RX CC:14 initializes both setpoint and applied_limit
        state.driver.update_raw(|raw| {
            raw.state = Some("C".to_string());
            raw.cc = Some(14);
        });
        state.sync_from_driver();
        let snap1 = manager.snapshot();
        assert_eq!(snap1.connectors[0].control.setpoint_amps, 14);
        assert_eq!(snap1.connectors[0].control.applied_limit_amps, 14);

        // 2. TX CC:20 sets setpoint_amps to 20 while applied_limit_amps remains 14 until RX confirms
        state.set_desired_limit_amps(20);
        let snap2 = manager.snapshot();
        assert_eq!(snap2.connectors[0].control.setpoint_amps, 20);
        assert_eq!(snap2.connectors[0].control.applied_limit_amps, 14);

        // 3. RX CC:10 (station clamped) updates applied_limit_amps to 10 while setpoint_amps stays 20
        state.driver.update_raw(|raw| {
            raw.cc = Some(10);
        });
        state.sync_from_driver();
        let snap3 = manager.snapshot();
        assert_eq!(snap3.connectors[0].control.setpoint_amps, 20);
        assert_eq!(snap3.connectors[0].control.applied_limit_amps, 10);
    }
}
