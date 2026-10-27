use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SerialStatus {
    /// Aucun adaptateur USB détecté / câble USB débranché du Raspberry Pi
    UsbDisconnected,
    /// Port USB ouvert, en attente de la première réponse de la borne
    UsbConnecting,
    /// Liaison USB active et saine (dialogue bidirectionnel avec l'ATmega)
    UsbConnected,
    /// Port USB ouvert mais aucune réponse reçue (borne éteinte ou ATmega figé)
    UsbNoResponse,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SerialConnection {
    pub status: SerialStatus,
    pub port: String,
    pub message: String,
    pub last_rx_secs_ago: Option<u64>,
}

impl Default for SerialConnection {
    fn default() -> Self {
        Self {
            status: SerialStatus::UsbDisconnected,
            port: String::from("/dev/ttyUSB0"),
            message: String::from("Câble USB non détecté (/dev/ttyUSB0 introuvable)"),
            last_rx_secs_ago: None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Telemetry {
    pub serial: SerialConnection,
    pub voltage: f32,
    pub current: f32,
    pub power: f32, // Calculated: V * I
    pub energy: f32,
    pub frequency: f32,
    pub error_code: String,
    pub limit_amps: u32,
    pub eliot_limit_amps: Option<u32>,
    pub cp_voltage: Option<u32>,
    #[serde(skip)]
    pub t2c_enabled: Option<bool>,
    #[serde(skip)]
    pub fm2_state: Option<bool>,
    #[serde(skip)]
    pub d_state: Option<bool>,
    pub charge_authorized: bool,
    pub charge_paused: bool,
    pub greenup_state: String,
    pub iec_state: Option<String>,
    pub charge_complete: bool,
    pub tic_mode: String, // "": unknown, "0": not present, "1200": historical, "9600": standard
    #[serde(skip)]
    pub last_power_update: Option<std::time::Instant>,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            serial: SerialConnection::default(),
            voltage: 0.0,
            current: 0.0,
            power: 0.0,
            energy: 0.0,
            frequency: 0.0,
            greenup_state: String::from("DISCONNECTED"),
            error_code: String::new(),
            limit_amps: 16, // 16A by default
            eliot_limit_amps: None,
            cp_voltage: None,
            t2c_enabled: None,
            fm2_state: None,
            d_state: None,
            charge_authorized: false,
            charge_paused: false,
            iec_state: None,
            charge_complete: false,
            tic_mode: String::new(),
            last_power_update: None,
        }
    }
}

use crate::hardware_specs::HardwareCapabilities;

#[derive(Default, Serialize, Deserialize, Clone, Debug)]
pub struct BoardInfo {
    pub software_version: String,
    pub hardware_version: String,
    pub serial_number: String,
    pub reference: String,
    pub week_year_production: String,
    pub bluetooth_enabled: Option<bool>,
    pub mac_address: Option<String>,
    pub side: Option<u8>,
    pub link_version: String,
    pub capabilities: Option<HardwareCapabilities>,
}
