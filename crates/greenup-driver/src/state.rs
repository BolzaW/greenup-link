use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, Mutex,
};
use tokio::sync::mpsc;
pub use crate::telemetry::{BoardInfo, SerialConnection, SerialStatus, Telemetry};

#[derive(Default)]
pub struct TicDetectionState {
    pub is_active: bool,
    pub zero_count: u32,
}

pub struct AppState {
    /// Charge point side: 1 for Side 1 (Right / Droit), 2 for Side 2 (Left / Gauche).
    pub side: AtomicU8,
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
        Self {
            side: AtomicU8::new(side),
            telemetry: Mutex::new(Telemetry::default()),
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

    /// Returns true if the USB serial link is active and actively communicating with the ATmega.
    pub fn is_serial_connected(&self) -> bool {
        self.telemetry
            .lock()
            .map(|t| t.serial.status == SerialStatus::UsbConnected)
            .unwrap_or(false)
    }
}

pub type SharedState = Arc<AppState>;

/// Manages multi-point charging stations (1 or 2 charge points).
pub struct StationManager {
    pub point1: SharedState,
    pub point2: Option<SharedState>,
}

impl StationManager {
    pub fn new(point1: SharedState, point2: Option<SharedState>) -> Self {
        Self { point1, point2 }
    }

    /// Returns true if two charging points are configured / active.
    pub fn is_dual(&self) -> bool {
        self.point2.is_some()
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
    pub fn get_point(&self, requested_side: Option<u8>) -> &SharedState {
        let target = requested_side.unwrap_or(1);
        if self.point1.side() == target {
            return &self.point1;
        }
        if let Some(ref p2) = self.point2 {
            if p2.side() == target {
                return p2;
            }
        }
        // Fallback by requested index if not yet assigned or both identical:
        if target == 2 {
            self.point2.as_ref().unwrap_or(&self.point1)
        } else {
            &self.point1
        }
    }
}

pub type SharedStation = Arc<StationManager>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_point_station_manager() {
        let (tx1, _rx1) = tokio::sync::mpsc::channel(1);
        let p1 = Arc::new(AppState::new_with_side(1, tx1));
        let manager = StationManager::new(p1.clone(), None);

        assert!(!manager.is_dual());
        assert_eq!(manager.available_points(), vec![1]);
        assert_eq!(manager.get_point(None).side(), 1);
        assert_eq!(manager.get_point(Some(1)).side(), 1);
        assert_eq!(manager.get_point(Some(2)).side(), 1); // fallback
    }

    #[test]
    fn test_dual_point_station_manager_dynamic_routing() {
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
        assert!(Arc::ptr_eq(manager.get_point(Some(1)), &p2));

        assert_eq!(manager.get_point(Some(2)).side(), 2);
        assert!(Arc::ptr_eq(manager.get_point(Some(2)), &p1));
    }
}
