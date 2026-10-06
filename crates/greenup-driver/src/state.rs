#[derive(Default)]
pub struct TicDetectionState {
    pub is_active: bool,
    pub zero_count: u32,
}

use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
pub use crate::telemetry::{BoardInfo, Telemetry};

pub struct AppState {
    pub telemetry: Mutex<Telemetry>,
    pub info: Mutex<BoardInfo>,
    // Canal utilisé par l'API pour envoyer des commandes au thread Série
    pub serial_tx: mpsc::Sender<String>,
    pub tic_detection: Mutex<TicDetectionState>,
}

pub type SharedState = Arc<AppState>;
