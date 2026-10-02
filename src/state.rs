use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Telemetry {
    pub voltage: f32,
    pub current: f32,
    pub power: f32, // Calculée: V * I
    pub energy: f32,
    pub frequency: f32,
    pub state: String,
    pub error_code: String,
    pub limit_amps: u32,
    pub charge_complete: bool,
    pub tic_mode: String, // "": inconnu, "0": non présent, "1200": historique, "9600": standard
    #[serde(skip)]
    pub last_power_update: Option<std::time::Instant>,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            voltage: 230.0, // Tension arbitraire par défaut pour le calcul de puissance
            current: 0.0,
            power: 0.0,
            energy: 0.0,
            frequency: 0.0,
            state: String::new(),
            error_code: String::new(),
            limit_amps: 16, // 16A par défaut
            charge_complete: false,
            tic_mode: String::new(),
            last_power_update: None,
        }
    }
}

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct BoardInfo {
    pub software_version: String,
    pub hardware_version: String,
    pub serial_number: String,
    pub reference: String,
    pub week_year_production: String,
    pub bluetooth_enabled: Option<bool>,
    pub link_version: String,
}

pub struct AppState {
    pub telemetry: Mutex<Telemetry>,
    pub info: Mutex<BoardInfo>,
    // Canal utilisé par l'API pour envoyer des commandes au thread Série
    pub serial_tx: mpsc::Sender<String>,
    pub tic_test_zero_count: Mutex<u32>,
}

// On définit un type plus court pour faciliter l'usage dans Axum
pub type SharedState = Arc<AppState>;
