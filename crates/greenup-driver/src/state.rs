use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
pub use crate::telemetry::{BoardInfo, SerialConnection, SerialStatus, Telemetry};

#[derive(Default)]
pub struct TicDetectionState {
    pub is_active: bool,
    pub zero_count: u32,
}

pub struct AppState {
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
        Self {
            telemetry: Mutex::new(Telemetry::default()),
            info: Mutex::new(BoardInfo::default()),
            serial_tx,
            tic_detection: Mutex::new(TicDetectionState::default()),
            tokio_handle: tokio::runtime::Handle::current(),
        }
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
