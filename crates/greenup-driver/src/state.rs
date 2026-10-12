use std::sync::{atomic::AtomicBool, Arc, Mutex};
use tokio::sync::mpsc;
pub use crate::telemetry::{BoardInfo, Telemetry};
pub use crate::custom_tic::CustomTicTelemetry;

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
    /// Dedicated parsed Linky TIC metrics received from custom firmware MacAdr:TIC responses.
    pub custom_tic: Mutex<CustomTicTelemetry>,
    /// Flag indicating whether the 10-second polling routine for custom TIC is active.
    pub custom_tic_active: AtomicBool,
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
            custom_tic: Mutex::new(CustomTicTelemetry::default()),
            custom_tic_active: AtomicBool::new(false),
            tokio_handle: tokio::runtime::Handle::current(),
        }
    }
}

pub type SharedState = Arc<AppState>;
