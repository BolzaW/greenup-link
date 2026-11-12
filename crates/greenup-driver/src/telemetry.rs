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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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
            message: String::from("USB cable not detected (/dev/ttyUSB0 not found)"),
            last_rx_secs_ago: None,
        }
    }
}

fn default_side() -> u8 {
    1
}

fn default_voltage_source() -> String {
    String::from("default")
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SupervisionMode {
    #[default]
    Remote,
    Local,
}

impl std::str::FromStr for SupervisionMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "remote" => Ok(SupervisionMode::Remote),
            "local" => Ok(SupervisionMode::Local),
            _ => Err(()),
        }
    }
}

impl SupervisionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            SupervisionMode::Remote => "remote",
            SupervisionMode::Local => "local",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        s.parse().ok()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalVoltage {
    pub voltage: f32,
    pub received_at: std::time::Instant,
    pub ttl_secs: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Telemetry {
    #[serde(default = "default_side")]
    pub side: u8,
    #[serde(default)]
    pub mode: SupervisionMode,
    pub serial: SerialConnection,
    pub voltage: f32,
    #[serde(default = "default_voltage_source")]
    pub voltage_source: String, // "default" or "external"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voltage_ttl_remaining_s: Option<u64>,
    #[serde(skip)]
    pub external_voltage: Option<ExternalVoltage>,
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
            side: 1,
            mode: SupervisionMode::default(),
            serial: SerialConnection::default(),
            voltage: 230.0,
            voltage_source: default_voltage_source(),
            voltage_ttl_remaining_s: None,
            external_voltage: None,
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

impl Telemetry {
    /// Re-evaluates effective voltage, source, and remaining TTL based on external voltage expiration.
    /// If an external voltage was set and hasn't expired, it takes priority over 230V.
    /// If expired (or none set), falls back to 230V (or 0.0V if USB disconnected).
    pub fn evaluate_voltage(&mut self) -> f32 {
        if let Some(ref ext) = self.external_voltage {
            let elapsed = ext.received_at.elapsed();
            let ttl = std::time::Duration::from_secs(ext.ttl_secs);
            if elapsed <= ttl {
                self.voltage = ext.voltage;
                self.voltage_source = "external".to_string();
                self.voltage_ttl_remaining_s = Some((ttl - elapsed).as_secs());
                return self.voltage;
            }
        }
        // Fallback to default or 0.0 if USB disconnected
        if self.serial.status == SerialStatus::UsbDisconnected {
            self.voltage = 0.0;
        } else {
            self.voltage = 230.0;
        }
        self.voltage_source = "default".to_string();
        self.voltage_ttl_remaining_s = None;
        self.voltage
    }

    /// Sets or updates the external voltage with a TTL in seconds.
    /// A ttl_secs of 0 immediately clears the external voltage and falls back to default 230V.
    pub fn set_external_voltage(&mut self, voltage: f32, ttl_secs: u64) {
        if ttl_secs == 0 {
            self.external_voltage = None;
        } else {
            self.external_voltage = Some(ExternalVoltage {
                voltage,
                received_at: std::time::Instant::now(),
                ttl_secs,
            });
        }
        self.evaluate_voltage();
        if self.greenup_state == "D" || self.greenup_state == "E" {
            self.power = self.voltage * self.current;
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
    #[serde(default)]
    pub is_dual: bool,
    #[serde(default)]
    pub available_sides: Vec<u8>,
}

/// Standard OCPP 1.6-J ChargePointStatus enum values.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum OcppChargePointStatus {
    Available,
    Preparing,
    Charging,
    SuspendedEVSE,
    SuspendedEV,
    Faulted,
    Unavailable,
}

/// Standardized charge point state compliant with IEC 61851-1 and OCPP 1.6-J.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConnectorStatus {
    pub ocpp: OcppChargePointStatus,
    pub iec: String,
    pub raw_code: String,
    pub vehicle_connected: bool,
    pub charging: bool,
    pub paused: bool,
    pub charge_complete: bool,
    pub error_code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

/// Dynamic charge limits and control setpoint in Amperes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConnectorControl {
    pub setpoint_amps: u32,
    pub pilot_limit_amps: u32,
    pub min_allowed_amps: u32,
    pub max_hardware_amps: u32,
    pub charge_authorized: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pilot_voltage: Option<u32>,
}

/// Normalized electrical metering values (Watts, kWh, Volts, Amperes).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConnectorMeter {
    pub power_w: f32,
    pub energy_kwh: f32,
    pub energy_total_wh: f32,
    pub current_a: f32,
    pub voltage_v: f32,
    pub voltage_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voltage_ttl_remaining_s: Option<u64>,
    pub frequency_hz: f32,
}

/// Grid interface telemetry (Linky TIC Tele-information).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConnectorGrid {
    pub tic_connected: bool,
    pub tic_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tic_baud_rate: Option<u32>,
}

/// Complete representation of an EVSE charge connector.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConnectorData {
    pub id: u8,
    pub status: ConnectorStatus,
    pub control: ConnectorControl,
    pub meter: ConnectorMeter,
    pub grid: ConnectorGrid,
}

/// Global metadata and system status of the charging station.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StationData {
    pub vendor: String,
    pub model: String,
    pub serial_number: String,
    pub hardware_version: String,
    pub firmware_version: String,
    pub link_version: String,
    pub supervision_mode: SupervisionMode,
    pub serial_connection: SerialConnection,
    pub is_dual: bool,
    pub available_connectors: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bluetooth_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<HardwareCapabilities>,
}

/// Unified station snapshot aggregating station metadata and all active connectors.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StationSnapshot {
    pub station: StationData,
    pub connectors: Vec<ConnectorData>,
}

impl Telemetry {
    /// Maps internal telemetry and board info to a standardized, OCPP-ready ConnectorData representation.
    pub fn to_connector_data(&self, info: &BoardInfo) -> ConnectorData {
        let raw_code = self.greenup_state.clone();
        let error_code = self.error_code.clone();
        let has_error = !error_code.is_empty() && error_code != "0000";

        // Determine IEC state
        let iec = match raw_code.as_str() {
            "A" | "DISCONNECTED" => "A".to_string(),
            "B" => "B".to_string(),
            "C" => "C".to_string(),
            "D" => "D".to_string(),
            "E" => "E".to_string(),
            other => self.iec_state.clone().unwrap_or_else(|| other.to_string()),
        };

        let vehicle_connected = iec != "A" && iec != "DISCONNECTED";
        let is_charging = (iec == "C" || iec == "D") && self.current > 0.5 && !self.charge_paused;

        let ocpp = if self.serial.status != SerialStatus::UsbConnected || self.mode == SupervisionMode::Local {
            if self.mode == SupervisionMode::Local && vehicle_connected {
                if is_charging {
                    OcppChargePointStatus::Charging
                } else {
                    OcppChargePointStatus::Preparing
                }
            } else if self.serial.status != SerialStatus::UsbConnected {
                OcppChargePointStatus::Unavailable
            } else {
                OcppChargePointStatus::Available
            }
        } else if has_error {
            OcppChargePointStatus::Faulted
        } else if self.charge_paused {
            OcppChargePointStatus::SuspendedEVSE
        } else if is_charging {
            OcppChargePointStatus::Charging
        } else if vehicle_connected {
            if self.charge_complete {
                OcppChargePointStatus::SuspendedEV
            } else {
                OcppChargePointStatus::Preparing
            }
        } else {
            OcppChargePointStatus::Available
        };

        let error_description = if has_error {
            crate::hardware_specs::error_code_description(&error_code).map(String::from)
        } else {
            None
        };

        let status = ConnectorStatus {
            ocpp,
            iec,
            raw_code,
            vehicle_connected,
            charging: is_charging,
            paused: self.charge_paused,
            charge_complete: self.charge_complete,
            error_code,
            error_description,
        };

        let max_hardware = info.capabilities.as_ref().map(|c| c.max_current_amps as u32).unwrap_or(32);
        let setpoint = self.eliot_limit_amps.unwrap_or(self.limit_amps).clamp(7, max_hardware);

        let control = ConnectorControl {
            setpoint_amps: setpoint,
            pilot_limit_amps: self.limit_amps,
            min_allowed_amps: 7,
            max_hardware_amps: max_hardware,
            charge_authorized: self.charge_authorized && !self.charge_paused,
            pilot_voltage: self.cp_voltage,
        };

        let meter = ConnectorMeter {
            power_w: self.power,
            energy_kwh: (self.energy / 1000.0 * 1000.0).round() / 1000.0,
            energy_total_wh: self.energy,
            current_a: self.current,
            voltage_v: self.voltage,
            voltage_source: self.voltage_source.clone(),
            voltage_ttl_remaining_s: self.voltage_ttl_remaining_s,
            frequency_hz: self.frequency,
        };

        let (tic_connected, tic_mode, tic_baud_rate) = match self.tic_mode.as_str() {
            "9600" => (true, "standard".to_string(), Some(9600)),
            "1200" => (true, "historic".to_string(), Some(1200)),
            "0" => (false, "none".to_string(), None),
            _ => (false, "unknown".to_string(), None),
        };

        let grid = ConnectorGrid {
            tic_connected,
            tic_mode,
            tic_baud_rate,
        };

        ConnectorData {
            id: self.side,
            status,
            control,
            meter,
            grid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_external_voltage_lifecycle() {
        let mut tel = Telemetry::default();
        tel.serial.status = SerialStatus::UsbConnected;
        assert_eq!(tel.evaluate_voltage(), 230.0);
        assert_eq!(tel.voltage_source, "default");
        assert_eq!(tel.voltage_ttl_remaining_s, None);

        // Inject external voltage (e.g. 238.4V with 60s TTL)
        tel.set_external_voltage(238.4, 60);
        assert_eq!(tel.voltage, 238.4);
        assert_eq!(tel.voltage_source, "external");
        assert!(tel.voltage_ttl_remaining_s.is_some());

        // While charging, power recalculates immediately
        tel.greenup_state = "D".to_string();
        tel.current = 16.0;
        tel.set_external_voltage(240.0, 60);
        assert_eq!(tel.power, 3840.0);

        // Clear external voltage with ttl_secs = 0
        tel.set_external_voltage(0.0, 0);
        assert_eq!(tel.voltage, 230.0);
        assert_eq!(tel.voltage_source, "default");
        assert_eq!(tel.voltage_ttl_remaining_s, None);
        assert_eq!(tel.power, 3680.0); // 230.0 * 16.0
    }

    #[test]
    fn test_external_voltage_expiration() {
        let mut tel = Telemetry::default();
        tel.serial.status = SerialStatus::UsbConnected;

        // Set with expired timestamp (1s TTL, timestamp 2s in the past)
        tel.external_voltage = Some(ExternalVoltage {
            voltage: 245.0,
            received_at: std::time::Instant::now() - std::time::Duration::from_secs(2),
            ttl_secs: 1,
        });

        assert_eq!(tel.evaluate_voltage(), 230.0);
        assert_eq!(tel.voltage_source, "default");
        assert_eq!(tel.voltage_ttl_remaining_s, None);
    }

    #[test]
    fn test_connector_data_mapping() {
        let info = BoardInfo {
            reference: "058001".to_string(),
            software_version: "1.03.25".to_string(),
            serial_number: "SIM05800101".to_string(),
            capabilities: Some(crate::hardware_specs::HardwareCapabilities::from_reference("058001")),
            ..Default::default()
        };

        let mut tel = Telemetry::default();
        tel.serial.status = SerialStatus::UsbConnected;

        // 1. Initial State A / Disconnected
        tel.greenup_state = "A".to_string();
        let conn = tel.to_connector_data(&info);
        assert_eq!(conn.id, 1);
        assert_eq!(conn.status.iec, "A");
        assert_eq!(conn.status.ocpp, OcppChargePointStatus::Available);
        assert!(!conn.status.vehicle_connected);
        assert!(!conn.status.charging);
        assert_eq!(conn.control.setpoint_amps, 16);
        assert_eq!(conn.control.max_hardware_amps, 32);

        // 2. Vehicle plugged in (State B)
        tel.greenup_state = "B".to_string();
        let conn = tel.to_connector_data(&info);
        assert_eq!(conn.status.iec, "B");
        assert_eq!(conn.status.ocpp, OcppChargePointStatus::Preparing);
        assert!(conn.status.vehicle_connected);
        assert!(!conn.status.charging);

        // 3. Charging active (State C, 16A)
        tel.greenup_state = "C".to_string();
        tel.current = 15.9;
        tel.power = 3736.0;
        let conn = tel.to_connector_data(&info);
        assert_eq!(conn.status.iec, "C");
        assert_eq!(conn.status.ocpp, OcppChargePointStatus::Charging);
        assert!(conn.status.vehicle_connected);
        assert!(conn.status.charging);
        assert_eq!(conn.meter.power_w, 3736.0);
        assert_eq!(conn.meter.current_a, 15.9);

        // 4. Charge paused
        tel.charge_paused = true;
        let conn = tel.to_connector_data(&info);
        assert_eq!(conn.status.ocpp, OcppChargePointStatus::SuspendedEVSE);
        assert!(conn.status.paused);
        assert!(!conn.status.charging);
        assert!(!conn.status.charge_complete);
        tel.charge_paused = false;

        // 4b. Charge complete (vehicle battery full, vehicle connected)
        tel.greenup_state = "B".to_string();
        tel.charge_complete = true;
        let conn_complete = tel.to_connector_data(&info);
        assert_eq!(conn_complete.status.ocpp, OcppChargePointStatus::SuspendedEV);
        assert!(conn_complete.status.charge_complete);
        tel.charge_complete = false;

        // 5. Hardware Fault code 0003
        tel.error_code = "0003".to_string();
        let conn = tel.to_connector_data(&info);
        assert_eq!(conn.status.ocpp, OcppChargePointStatus::Faulted);
        assert_eq!(conn.status.error_code, "0003");
        assert_eq!(conn.status.error_description, Some("Control pilot short-circuit (cable/vehicle)".to_string()));
        tel.error_code.clear();

        // 6. TIC mode 9600
        tel.tic_mode = "9600".to_string();
        let conn = tel.to_connector_data(&info);
        assert!(conn.grid.tic_connected);
        assert_eq!(conn.grid.tic_mode, "standard");
        assert_eq!(conn.grid.tic_baud_rate, Some(9600));
    }
}
