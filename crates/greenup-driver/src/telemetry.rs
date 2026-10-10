use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Telemetry {
    pub voltage: f32,
    pub current: f32,
    pub power: f32, // Calculated: V * I
    pub energy: f32,
    pub frequency: f32,
    pub greenup_state: String,
    pub error_code: String,
    pub limit_amps: u32,
    pub eliot_limit_amps: Option<u32>,
    pub cp_voltage: Option<u32>,
    pub t2c_enabled: Option<bool>,
    pub sb_state: Option<bool>,
    pub fm2_state: Option<bool>,
    pub d_state: Option<bool>,
    pub charge_authorized: bool,
    pub charge_paused: bool,
    pub iec_state: Option<String>,
    pub charge_complete: bool,
    pub tic_mode: String, // "": unknown, "0": not present, "1200": historical, "9600": standard
    #[serde(skip)]
    pub last_power_update: Option<std::time::Instant>,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            voltage: 230.0, // Arbitrary default voltage for power calculation
            current: 0.0,
            power: 0.0,
            energy: 0.0,
            frequency: 0.0,
            greenup_state: String::new(),
            error_code: String::new(),
            limit_amps: 16, // 16A by default
            eliot_limit_amps: None,
            cp_voltage: None,
            t2c_enabled: None,
            sb_state: None,
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
    pub link_version: String,
    pub capabilities: Option<HardwareCapabilities>,
}
