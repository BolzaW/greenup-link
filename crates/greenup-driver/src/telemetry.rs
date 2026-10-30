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
}
