use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
pub use greenup_protocol::{BoardInfo, Telemetry};

pub struct AppState {
    pub telemetry: Mutex<Telemetry>,
    pub info: Mutex<BoardInfo>,
    // Canal utilisé par l'API pour envoyer des commandes au thread Série
    pub serial_tx: mpsc::Sender<String>,
    pub tic_test_zero_count: Mutex<u32>,
}

pub type SharedState = Arc<AppState>;
