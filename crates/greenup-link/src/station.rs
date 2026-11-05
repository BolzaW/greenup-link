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
    atomic::{AtomicU8, Ordering},
    Arc, Mutex, RwLock,
};
use tokio::sync::mpsc;

use crate::hardware_specs::HardwareCapabilities;
pub use crate::telemetry::{
    BoardInfo, ConnectorData, OcppChargePointStatus, SerialConnection, SerialStatus, StationData,
    StationSnapshot, SupervisionMode, Telemetry,
};
use greenup_driver::{
    logger, DriverHandle, DriverSnapshot, SerialOptions, SharedDriver, TicStartError,
};
use greenup_protocol::Command;

#[derive(Default, Debug)]
struct SyncCursor {
    last_seq: u64,
    last_state: String,
    last_cci_count: u64,
    last_ticket_count: u64,
    last_cph: Option<String>,
    auto_tic_triggered: bool,
}

/// High-level supervisor state for a single charge point (Side 1 or Side 2).
/// Wraps a low-level [`SharedDriver`] and maintains the interpreted domain model.
pub struct AppState {
    /// Underlying low-level serial/USB driver handle.
    pub driver: SharedDriver,
    /// Charge point side: 1 for Side 1 (Right / Droit), 2 for Side 2 (Left / Gauche).
    pub side: AtomicU8,
    /// Supervision mode: 0 for Remote (active), 1 for Local (passive).
    pub mode: AtomicU8,
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
        Self::new_with_side_and_mode(side, SupervisionMode::Remote, serial_tx)
    }

    pub fn new_with_side_and_mode(
        side: u8,
        mode: SupervisionMode,
        serial_tx: mpsc::Sender<Command>,
    ) -> Self {
        let driver = Arc::new(DriverHandle::new_with_side_and_mode(
            side,
            mode,
            serial_tx,
        ));
        Self::from_driver(driver, env!("CARGO_PKG_VERSION"))
    }

    /// Creates a high-level `AppState` attached to an existing low-level [`SharedDriver`].
    pub fn from_driver(driver: SharedDriver, link_version: &str) -> Self {
        let side = driver.side();
        let mode = driver.mode();
        let serial_tx = driver.serial_tx.clone();

        let mut tel = Telemetry::default();
        tel.side = side;
        tel.mode = mode;

        let mut info = BoardInfo::default();
        info.side = Some(side);
        info.link_version = link_version.to_string();

        let mode_val = match mode {
            SupervisionMode::Remote => 0,
            SupervisionMode::Local => 1,
        };

        Self {
            driver,
            side: AtomicU8::new(side),
            mode: AtomicU8::new(mode_val),
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
            }

            // 3. Update Telemetry, State transitions, Power/Energy, and Sessions
            {
                let mut tel = self.telemetry.lock().unwrap_or_else(|e| e.into_inner());
                tel.side = side;
                tel.mode = usb.mode;
                self.mode.store(
                    match usb.mode {
                        SupervisionMode::Remote => 0,
                        SupervisionMode::Local => 1,
                    },
                    Ordering::Relaxed,
                );

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
                if let Some(fm2) = raw.fm2 {
                    tel.fm2_state = Some(fm2);
                }
                if let Some(d) = raw.d {
                    tel.d_state = Some(d);
                }
                if let Some(sb) = raw.sb {
                    tel.charge_paused = !sb;
                } else if let Some(sbf) = raw.sbf {
                    tel.charge_paused = !sbf;
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
                        } else if state_val == "A" {
                            tel.current = 0.0;
                            tel.power = 0.0;
                            tel.last_power_update = None;

                            if crate::session::is_active_for_side(side) {
                                crate::session::on_state_change_for_side(side, state_val, &tel);
                            } else {
                                tel.energy = 0.0;
                                tel.charge_complete = false;
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
                    let c = raw.cci.unwrap_or(0.0);
                    tel.evaluate_voltage();
                    if matches!(tel.greenup_state.as_str(), "D" | "E" | "G" | "H") {
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
                    crate::session::on_telemetry_tick_for_side(side, &tel);
                }

                if raw.wt_updates_count > cursor.last_ticket_count {
                    cursor.last_ticket_count = raw.wt_updates_count;
                    if let Some(ref ticket) = raw.wt {
                        crate::session::record_ticket_for_side(side, ticket);
                    }
                }
            }

            // 4. High-level automatic TIC detection check after driver init completes
            if !usb.init_completed {
                cursor.auto_tic_triggered = false;
            } else if usb.init_completed
                && !cursor.auto_tic_triggered
                && usb.mode == SupervisionMode::Remote
            {
                cursor.auto_tic_triggered = true;
                should_auto_start_tic = true;
            }
        }

        if should_auto_start_tic {
            match check_tic_eligibility(self) {
                Ok(()) => {
                    logger::log(
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

    /// Returns current active supervision mode (Remote or Local).
    pub fn mode(&self) -> SupervisionMode {
        if self.mode.load(Ordering::Relaxed) == 1 {
            SupervisionMode::Local
        } else {
            SupervisionMode::Remote
        }
    }

    /// Sets the supervision mode on both the high-level state and low-level driver.
    pub fn set_mode(&self, mode: SupervisionMode) {
        let mode_val = match mode {
            SupervisionMode::Remote => 0,
            SupervisionMode::Local => 1,
        };
        self.mode.store(mode_val, Ordering::Relaxed);
        if let Ok(mut tel) = self.telemetry.lock() {
            tel.mode = mode;
        }
        self.driver.set_mode(mode);
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
/// and initiates TIC detection if the station hardware model supports TIC and is in `State:A`.
pub async fn trigger_init_sequence(state: &SharedState) {
    greenup_driver::trigger_init_sequence(&state.driver).await;
    let _ = start_tic_detection(state).await;
}

/// Manages multi-point charging stations (1 or 2 charge points).
pub struct StationManager {
    pub point1: SharedState,
    pub point2: RwLock<Option<SharedState>>,
}

impl StationManager {
    pub fn new(point1: SharedState, point2: Option<SharedState>) -> Self {
        Self {
            point1,
            point2: RwLock::new(point2),
        }
    }

    /// Attaches the second charge point dynamically after startup.
    pub fn attach_point2(&self, p2: SharedState) {
        let mut guard = self.point2.write().unwrap_or_else(|e| e.into_inner());
        *guard = Some(p2);
    }

    /// Watches Point 1's low-level driver via `tokio::sync::watch` and automatically requests
    /// `greenup-driver` to discover and initialize the 2nd USB port if the Legrand reference
    /// corresponds to a Dual Charge Point (`charging_points == 2`).
    pub fn watch_auto_dual_port(
        self: &Arc<Self>,
        configured_port1: Option<String>,
        configured_port2: Option<String>,
        baud_rate: u32,
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
                            let point_mode = station.point1.mode();
                            let driver2 = greenup_driver::spawn_port(
                                2,
                                point_mode,
                                SerialOptions {
                                    port: port2_target,
                                    baud_rate,
                                    timeout_ms: 100,
                                    watchdog_timeout_secs,
                                },
                            );
                            let app_state_2 = Arc::new(AppState::from_driver(driver2, &link_version));
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
            supervision_mode: self.point1.mode(),
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
        };

        StationSnapshot {
            station,
            connectors,
        }
    }
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
        assert_eq!(snap.connectors[0].control.setpoint_amps, 16);
        assert_eq!(snap.connectors[0].control.pilot_limit_amps, 20);
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
}
