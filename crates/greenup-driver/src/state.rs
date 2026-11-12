use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, Mutex, RwLock,
};
use tokio::sync::mpsc;
pub use crate::telemetry::{BoardInfo, SerialConnection, SerialStatus, SupervisionMode, Telemetry};

#[derive(Default)]
pub struct TicDetectionState {
    pub is_active: bool,
    pub zero_count: u32,
}

pub struct AppState {
    /// Charge point side: 1 for Side 1 (Right / Droit), 2 for Side 2 (Left / Gauche).
    pub side: AtomicU8,
    /// Supervision mode: 0 for Remote (active), 1 for Local (passive).
    pub mode: AtomicU8,
    pub telemetry: Mutex<Telemetry>,
    pub info: Mutex<BoardInfo>,
    // Channel used by the API to send commands to the Serial thread
    pub serial_tx: mpsc::Sender<greenup_protocol::Command>,
    pub tic_detection: Mutex<TicDetectionState>,
    /// Tokio runtime handle to spawn background tasks from driver threads.
    pub tokio_handle: tokio::runtime::Handle,
}

impl AppState {
    pub fn new(serial_tx: mpsc::Sender<greenup_protocol::Command>) -> Self {
        Self::new_with_side(1, serial_tx)
    }

    pub fn new_with_side(side: u8, serial_tx: mpsc::Sender<greenup_protocol::Command>) -> Self {
        Self::new_with_side_and_mode(side, SupervisionMode::Remote, serial_tx)
    }

    pub fn new_with_side_and_mode(
        side: u8,
        mode: SupervisionMode,
        serial_tx: mpsc::Sender<greenup_protocol::Command>,
    ) -> Self {
        let mut tel = Telemetry::default();
        tel.side = side;
        tel.mode = mode;
        let mode_val = match mode {
            SupervisionMode::Remote => 0,
            SupervisionMode::Local => 1,
        };
        Self {
            side: AtomicU8::new(side),
            mode: AtomicU8::new(mode_val),
            telemetry: Mutex::new(tel),
            info: Mutex::new(BoardInfo::default()),
            serial_tx,
            tic_detection: Mutex::new(TicDetectionState::default()),
            tokio_handle: tokio::runtime::Handle::current(),
        }
    }

    /// Returns current active side number (1 or 2).
    pub fn side(&self) -> u8 {
        self.side.load(Ordering::Relaxed)
    }

    /// Sets the side number once discovered or reconfigured.
    pub fn set_side(&self, s: u8) {
        self.side.store(s, Ordering::Relaxed);
    }

    /// Returns current active supervision mode (Remote or Local).
    pub fn mode(&self) -> SupervisionMode {
        if self.mode.load(Ordering::Relaxed) == 1 {
            SupervisionMode::Local
        } else {
            SupervisionMode::Remote
        }
    }

    /// Sets the supervision mode and synchronizes with telemetry.
    pub fn set_mode(&self, mode: SupervisionMode) {
        let mode_val = match mode {
            SupervisionMode::Remote => 0,
            SupervisionMode::Local => 1,
        };
        self.mode.store(mode_val, Ordering::Relaxed);
        if let Ok(mut tel) = self.telemetry.lock() {
            tel.mode = mode;
        }
    }

    /// Returns true if the USB serial link is active and actively communicating with the ATmega.
    pub fn is_serial_connected(&self) -> bool {
        self.telemetry
            .lock()
            .map(|t| t.serial.status == SerialStatus::UsbConnected)
            .unwrap_or(false)
    }

    /// Converts the current charge point state into a standardized ConnectorData representation.
    pub fn to_connector_data(&self) -> crate::telemetry::ConnectorData {
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

    /// Returns true if two charging points are configured / active.
    pub fn is_dual(&self) -> bool {
        self.point2.read().unwrap_or_else(|e| e.into_inner()).is_some()
    }

    /// Returns the list of active point numbers: [1] for single-point, [1, 2] for dual-point.
    pub fn available_points(&self) -> Vec<u8> {
        if self.is_dual() {
            vec![1, 2]
        } else {
            vec![1]
        }
    }

    /// Resolves the state for the given side (1 or 2).
    /// Dynamically routes to the charge point whose ATmega reported that side,
    /// or falls back cleanly to point1 / point2.
    pub fn get_point(&self, requested_side: Option<u8>) -> SharedState {
        let target = requested_side.unwrap_or(1);
        if self.point1.side() == target {
            return self.point1.clone();
        }
        let guard = self.point2.read().unwrap_or_else(|e| e.into_inner());
        if let Some(ref p2) = *guard {
            if p2.side() == target {
                return p2.clone();
            }
        }
        // Fallback by requested index if not yet assigned or both identical:
        if target == 2 {
            guard.as_ref().cloned().unwrap_or_else(|| self.point1.clone())
        } else {
            self.point1.clone()
        }
    }

    /// Produces a complete, unified StationSnapshot aggregating station-level
    /// metadata and all active charge point connectors (OCPP & EVCC aligned).
    pub fn snapshot(&self) -> crate::telemetry::StationSnapshot {
        let p1_info = self.point1.info.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let p1_tel = {
            let mut tel = self.point1.telemetry.lock().unwrap_or_else(|e| e.into_inner());
            tel.evaluate_voltage();
            tel.clone()
        };

        let mut connectors = vec![p1_tel.to_connector_data(&p1_info)];

        let is_dual = self.is_dual();
        let available_connectors = self.available_points();

        let guard = self.point2.read().unwrap_or_else(|e| e.into_inner());
        if let Some(ref p2) = *guard {
            let p2_info = p2.info.lock().unwrap_or_else(|e| e.into_inner()).clone();
            let p2_tel = {
                let mut tel = p2.telemetry.lock().unwrap_or_else(|e| e.into_inner());
                tel.evaluate_voltage();
                tel.clone()
            };
            connectors.push(p2_tel.to_connector_data(&p2_info));
        }

        let station = crate::telemetry::StationData {
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

        crate::telemetry::StationSnapshot {
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
    async fn test_single_point_station_manager() {
        let (tx1, _rx1) = tokio::sync::mpsc::channel(1);
        let p1 = Arc::new(AppState::new_with_side(1, tx1));
        let manager = StationManager::new(p1.clone(), None);

        assert!(!manager.is_dual());
        assert_eq!(manager.available_points(), vec![1]);
        assert_eq!(manager.get_point(None).side(), 1);
        assert_eq!(manager.get_point(Some(1)).side(), 1);
        assert_eq!(manager.get_point(Some(2)).side(), 1); // fallback
    }

    #[tokio::test]
    async fn test_dual_point_station_manager_dynamic_routing() {
        let (tx1, _rx1) = tokio::sync::mpsc::channel(1);
        let (tx2, _rx2) = tokio::sync::mpsc::channel(1);
        let p1 = Arc::new(AppState::new_with_side(1, tx1));
        let p2 = Arc::new(AppState::new_with_side(2, tx2));
        let manager = StationManager::new(p1.clone(), Some(p2.clone()));

        assert!(manager.is_dual());
        assert_eq!(manager.available_points(), vec![1, 2]);
        assert_eq!(manager.get_point(Some(1)).side(), 1);
        assert_eq!(manager.get_point(Some(2)).side(), 2);

        // Inverted USB detection test:
        // /dev/ttyUSB0 turns out to be Side 2, and /dev/ttyUSB1 turns out to be Side 1
        p1.set_side(2);
        p2.set_side(1);

        assert_eq!(manager.get_point(Some(1)).side(), 1);
        assert!(Arc::ptr_eq(&manager.get_point(Some(1)), &p2));

        assert_eq!(manager.get_point(Some(2)).side(), 2);
        assert!(Arc::ptr_eq(&manager.get_point(Some(2)), &p1));

        // Test dynamic attach
        let manager_dyn = StationManager::new(p1.clone(), None);
        assert!(!manager_dyn.is_dual());
        manager_dyn.attach_point2(p2.clone());
        assert!(manager_dyn.is_dual());
        assert_eq!(manager_dyn.available_points(), vec![1, 2]);
    }

    #[tokio::test]
    async fn test_app_state_supervision_mode() {
        use crate::telemetry::SupervisionMode;
        let (tx, _rx) = tokio::sync::mpsc::channel(1);
        let state = AppState::new_with_side_and_mode(1, SupervisionMode::Remote, tx);
        assert_eq!(state.mode(), SupervisionMode::Remote);

        state.set_mode(SupervisionMode::Local);
        assert_eq!(state.mode(), SupervisionMode::Local);

        state.set_mode(SupervisionMode::Remote);
        assert_eq!(state.mode(), SupervisionMode::Remote);
    }

    #[tokio::test]
    async fn test_station_manager_snapshot() {
        use crate::telemetry::{OcppChargePointStatus, SerialStatus};
        let (tx1, _rx1) = tokio::sync::mpsc::channel(1);
        let (tx2, _rx2) = tokio::sync::mpsc::channel(1);
        let p1 = Arc::new(AppState::new_with_side(1, tx1));
        let p2 = Arc::new(AppState::new_with_side(2, tx2));

        {
            let mut info1 = p1.info.lock().unwrap();
            info1.reference = "058001".to_string();
            info1.serial_number = "SIM01".to_string();
            info1.hardware_version = "A".to_string();
            info1.software_version = "1.03.25".to_string();
            info1.link_version = "0.5.0".to_string();
            let mut tel1 = p1.telemetry.lock().unwrap();
            tel1.serial.status = SerialStatus::UsbConnected;
            tel1.greenup_state = "A".to_string();
        }

        {
            let mut info2 = p2.info.lock().unwrap();
            info2.reference = "058001".to_string();
            info2.serial_number = "SIM02".to_string();
            let mut tel2 = p2.telemetry.lock().unwrap();
            tel2.serial.status = SerialStatus::UsbConnected;
            tel2.greenup_state = "B".to_string();
        }

        // 1. Single point station snapshot
        let manager_single = StationManager::new(p1.clone(), None);
        let snap1 = manager_single.snapshot();
        assert_eq!(snap1.station.vendor, "Legrand");
        assert_eq!(snap1.station.model, "058001");
        assert_eq!(snap1.station.serial_number, "SIM01");
        assert!(!snap1.station.is_dual);
        assert_eq!(snap1.connectors.len(), 1);
        assert_eq!(snap1.connectors[0].id, 1);
        assert_eq!(snap1.connectors[0].status.ocpp, OcppChargePointStatus::Available);

        // 2. Dual point station snapshot
        let manager_dual = StationManager::new(p1.clone(), Some(p2.clone()));
        let snap2 = manager_dual.snapshot();
        assert!(snap2.station.is_dual);
        assert_eq!(snap2.connectors.len(), 2);
        assert_eq!(snap2.connectors[0].id, 1);
        assert_eq!(snap2.connectors[0].status.ocpp, OcppChargePointStatus::Available);
        assert_eq!(snap2.connectors[1].id, 2);
        assert_eq!(snap2.connectors[1].status.ocpp, OcppChargePointStatus::Preparing);
    }
}
