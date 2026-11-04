//! Low-level JSON data models for `greenup-driver`:
//! - [`AtmegaRawState`]: Raw registers and telemetry reported by the ATmega2560 over UART.
//! - [`UsbLinkState`]: Physical USB/UART serial link health and counters managed by the driver.
//! - [`DriverSnapshot`]: Combined snapshot broadcast via `tokio::sync::watch` on every state change.

use greenup_protocol::ProtocolEvent;
use serde::{Deserialize, Serialize};

/// Physical and logical state of the USB serial connection to the ATmega board.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SerialStatus {
    /// No USB serial adapter detected or USB cable unplugged.
    #[default]
    UsbDisconnected,
    /// USB serial port opened, waiting for the first valid frame from the ATmega.
    UsbConnecting,
    /// Active bidirectional UART communication with the ATmega.
    UsbConnected,
    /// USB serial port opened, but the ATmega is not responding (board unpowered or frozen).
    UsbNoResponse,
}

/// Operating mode of the serial driver on the UART bus.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SupervisionMode {
    /// Active supervision mode (`RaspberryPiModeOK`, startup queries, watchdog pings, commands allowed).
    #[default]
    Remote,
    /// Passive monitoring mode (RX-only listen mode, zero spontaneous TX frames).
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

/// JSON-serializable state of the USB serial link managed by `greenup-driver`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct UsbLinkState {
    pub status: SerialStatus,
    pub port: String,
    pub baud_rate: u32,
    pub mode: SupervisionMode,
    pub message: String,
    pub last_rx_secs_ago: Option<u64>,
    pub init_completed: bool,
    pub rx_frames_count: u64,
    pub tx_commands_count: u64,
}

impl Default for UsbLinkState {
    fn default() -> Self {
        Self {
            status: SerialStatus::UsbDisconnected,
            port: String::from("/dev/ttyUSB0"),
            baud_rate: 115_200,
            mode: SupervisionMode::Remote,
            message: String::from("USB cable not detected (/dev/ttyUSB0 not found)"),
            last_rx_secs_ago: None,
            init_completed: false,
            rx_frames_count: 0,
            tx_commands_count: 0,
        }
    }
}

/// Pure, raw register mirror of the ATmega2560 power board.
///
/// Every field directly reflects the raw value reported by the ATmega over the serial
/// interface without high-level business assumptions. Fields remain `None` until the
/// ATmega explicitly transmits the corresponding frame.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct AtmegaRawState {
    // --- 1. System Identification & Boot Handshake ---
    pub kit_com: Option<bool>,
    pub initialized: Option<bool>,
    pub software_version: Option<String>,
    pub hardware_version: Option<String>,
    pub serial_number: Option<String>,
    pub reference: Option<String>,
    pub week_year_production: Option<String>,
    pub side_raw: Option<String>,
    pub side: Option<u8>,
    pub dip1: Option<String>,
    pub dip2: Option<String>,
    pub dip3: Option<String>,
    pub dip4: Option<String>,

    // --- 2. Charge Telemetry, State Machine & Hardware Sensors ---
    pub state: Option<String>,
    pub error: Option<String>,
    pub cp: Option<u32>,
    pub pp: Option<u32>,
    pub cc_ca: Option<u32>,
    pub cci: Option<f32>,
    pub cci_l2: Option<f32>,
    pub cci_l3: Option<f32>,
    pub cci_updates_count: u64,
    pub fcs: Option<bool>,
    pub dom: Option<bool>,
    pub sb: Option<bool>,
    pub sbf: Option<bool>,
    pub cph: Option<String>,

    // --- 3. Current & Power Modulation ---
    pub cc: Option<u32>,
    pub ccs: Option<u32>,
    pub cc_el: Option<u32>,
    pub cc_co: Option<u32>,
    pub cc_tic: Option<u32>,
    pub cc_sl1: Option<u32>,
    pub cc_sl2: Option<u32>,
    pub cc_sl3: Option<u32>,
    pub cc_sl4: Option<u32>,
    pub cc_bles: Option<u32>,
    pub cc_blesh: Option<u32>,

    // --- 4. Sockets & Actuator Hardware Control ---
    pub t2c: Option<bool>,
    pub dom_2pc: Option<bool>,
    pub t2f: Option<bool>,
    pub dom_2pf: Option<bool>,

    // --- 5. Planning, Scheduling & Modes ---
    pub pa: Option<bool>,
    pub pafc: Option<bool>,
    pub d: Option<bool>,
    pub dec: Option<u32>,
    pub fm: Option<String>,
    pub fm2: Option<bool>,
    pub cau: Option<bool>,

    // --- 6. OCPP Status ---
    pub ocpp_status: Option<String>,

    // --- 7. Bluetooth Controller (BLE) ---
    pub bt: Option<bool>,
    pub mac_adr: Option<String>,

    // --- 8. Diagnostic, TIC & Spontaneous Frames (WT, CPh, SBF) ---
    pub tic_tm: Option<bool>,
    pub tic_test_b: Option<String>,
    pub wt: Option<String>,
    pub wt_updates_count: u64,
}

impl AtmegaRawState {
    /// Decodes a raw `Side:` string from the ATmega (`"1"`, `"49"`, `"2"`, `"48"`, `"50"`, `"0"`)
    /// into a normalized side number (`1` or `2`).
    pub fn decode_side_number(raw: &str) -> Option<u8> {
        match raw.trim() {
            "49" | "1" => Some(1),
            "48" | "50" | "2" | "0" => Some(2),
            other => other.parse::<u8>().ok(),
        }
    }

    /// Applies a parsed [`ProtocolEvent`] to this raw register state.
    /// Returns `true` if any register or counter was updated.
    pub fn apply_event(&mut self, event: &ProtocolEvent) -> bool {
        match event {
            ProtocolEvent::Ping | ProtocolEvent::Pong | ProtocolEvent::Unknown(_) => false,

            // 1. Identity & Boot Handshake
            ProtocolEvent::KitCom(v) => {
                self.kit_com = Some(*v);
                true
            }
            ProtocolEvent::Initialized(v) => {
                self.initialized = Some(*v);
                true
            }
            ProtocolEvent::SoftwareVersion(v) => {
                self.software_version = Some(v.clone());
                true
            }
            ProtocolEvent::HardwareVersion(v) => {
                self.hardware_version = Some(v.clone());
                true
            }
            ProtocolEvent::SerialNumber(v) => {
                self.serial_number = Some(v.clone());
                true
            }
            ProtocolEvent::Reference(v) => {
                self.reference = Some(v.clone());
                true
            }
            ProtocolEvent::WeekYearProduction(v) => {
                self.week_year_production = Some(v.clone());
                true
            }
            ProtocolEvent::Side(v) => {
                self.side_raw = Some(v.clone());
                self.side = Self::decode_side_number(v);
                true
            }
            ProtocolEvent::DipSwitch { index, value } => {
                match index {
                    1 => self.dip1 = Some(value.clone()),
                    2 => self.dip2 = Some(value.clone()),
                    3 => self.dip3 = Some(value.clone()),
                    4 => self.dip4 = Some(value.clone()),
                    _ => return false,
                }
                true
            }

            // 2. Charge Telemetry, State Machine & Hardware Sensors
            ProtocolEvent::StateChange(s) => {
                let is_charging = matches!(s.as_str(), "D" | "E" | "G" | "H");
                if !is_charging {
                    self.cci = Some(0.0);
                    self.cci_l2 = None;
                    self.cci_l3 = None;
                }
                if s == "A" || matches!(s.as_str(), "L" | "R" | "ST" | "S" | "T" | "U" | "V" | "W" | "X" | "Y" | "Z") {
                    self.fcs = Some(false);
                }
                self.state = Some(s.clone());
                true
            }
            ProtocolEvent::ErrorChange(e) => {
                self.error = Some(e.clone());
                true
            }
            ProtocolEvent::CpVoltage(v) => {
                self.cp = Some(*v);
                true
            }
            ProtocolEvent::PpCurrent(v) => {
                self.pp = Some(*v);
                true
            }
            ProtocolEvent::CableCurrentCapacity(v) => {
                self.cc_ca = Some(*v);
                true
            }
            ProtocolEvent::InstantaneousCurrent { l1, l2, l3 } => {
                self.cci = Some(*l1);
                self.cci_l2 = *l2;
                self.cci_l3 = *l3;
                self.cci_updates_count = self.cci_updates_count.wrapping_add(1);
                true
            }
            ProtocolEvent::FullChargeState(b) => {
                self.fcs = Some(*b);
                true
            }
            ProtocolEvent::DomesticPlugPresent(b) => {
                self.dom = Some(*b);
                true
            }
            ProtocolEvent::SbState(b) => {
                self.sb = Some(*b);
                true
            }
            ProtocolEvent::SbfState(b) => {
                self.sbf = Some(*b);
                true
            }
            ProtocolEvent::ChargePhases(p) => {
                self.cph = Some(p.clone());
                true
            }

            // 3. Current & Power Modulation
            ProtocolEvent::LimitAmps(v) => {
                self.cc = Some(*v);
                true
            }
            ProtocolEvent::SupervisorLimitAmps(v) => {
                self.ccs = Some(*v);
                true
            }
            ProtocolEvent::EliotLimitAmps(v) => {
                self.cc_el = Some(*v);
                true
            }
            ProtocolEvent::ConsolidatedLimitAmps(v) => {
                self.cc_co = Some(*v);
                true
            }
            ProtocolEvent::TicLimitAmps(v) => {
                self.cc_tic = Some(*v);
                true
            }
            ProtocolEvent::SubLineLimitAmps { line, amps } => {
                match line {
                    1 => self.cc_sl1 = Some(*amps),
                    2 => self.cc_sl2 = Some(*amps),
                    3 => self.cc_sl3 = Some(*amps),
                    4 => self.cc_sl4 = Some(*amps),
                    _ => return false,
                }
                true
            }
            ProtocolEvent::BleStandardLimitAmps(v) => {
                self.cc_bles = Some(*v);
                true
            }
            ProtocolEvent::BleShuntLimitAmps(v) => {
                self.cc_blesh = Some(*v);
                true
            }

            // 4. Sockets & Actuator Hardware Control
            ProtocolEvent::T2CEnabled(b) => {
                self.t2c = Some(*b);
                true
            }
            ProtocolEvent::DomesticEnabled(b) => {
                self.dom_2pc = Some(*b);
                true
            }
            ProtocolEvent::ForceType2State(b) => {
                self.t2f = Some(*b);
                true
            }
            ProtocolEvent::ForceDomesticState(b) => {
                self.dom_2pf = Some(*b);
                true
            }

            // 5. Planning, Scheduling & Modes
            ProtocolEvent::PlanningAuthorization(b) => {
                self.pa = Some(*b);
                true
            }
            ProtocolEvent::PlanningAutoForceCharge(b) => {
                self.pafc = Some(*b);
                true
            }
            ProtocolEvent::Derogation(b) => {
                self.d = Some(*b);
                true
            }
            ProtocolEvent::DeferredStartDelay(m) => {
                self.dec = Some(*m);
                true
            }
            ProtocolEvent::FmMode(m) => {
                self.fm = Some(m.clone());
                true
            }
            ProtocolEvent::ExternalSignal(b) => {
                self.fm2 = Some(*b);
                true
            }
            ProtocolEvent::EmergencyStop(b) => {
                self.cau = Some(*b);
                true
            }

            // 6. OCPP Status
            ProtocolEvent::OcppStatus(v) => {
                self.ocpp_status = Some(v.clone());
                true
            }

            // 7. Bluetooth Controller (BLE)
            ProtocolEvent::BluetoothState(b) => {
                self.bt = Some(*b);
                true
            }
            ProtocolEvent::MacAddress(m) => {
                self.mac_adr = Some(m.clone());
                true
            }

            // 8. Diagnostic, TIC & Spontaneous Frames (WT, CPh, SBF)
            ProtocolEvent::TicTestMode(b) => {
                self.tic_tm = Some(*b);
                true
            }
            ProtocolEvent::TicTestBaud(b) => {
                self.tic_test_b = Some(b.clone());
                true
            }
            ProtocolEvent::SessionTicket(t) => {
                self.wt = Some(t.clone());
                self.wt_updates_count = self.wt_updates_count.wrapping_add(1);
                true
            }

            // Ignored acknowledgments and unsupported peripherals (Unlock, Sleep, EM, Modbus, RFID, OCPP config)
            _ => false,
        }
    }
}

/// Complete low-level snapshot of a single USB serial port and its connected ATmega.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct DriverSnapshot {
    /// Monotonically increasing sequence number incremented on every raw state or USB link update.
    pub seq: u64,
    /// Raw ATmega2560 registers and telemetry.
    pub atmega: AtmegaRawState,
    /// Physical USB serial link state.
    pub usb: UsbLinkState,
}

impl DriverSnapshot {
    /// Serializes this low-level driver snapshot to a `serde_json::Value`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use greenup_protocol::parse_line;

    #[test]
    fn test_atmega_raw_state_applies_protocol_events_and_serializes_to_json() {
        let mut raw = AtmegaRawState::default();
        assert!(raw.apply_event(&parse_line("KitCom:1")));
        assert!(raw.apply_event(&parse_line("Initialized:1")));
        assert!(raw.apply_event(&parse_line("Reference:058001")));
        assert!(raw.apply_event(&parse_line("Side:49")));
        assert!(raw.apply_event(&parse_line("DIP1:1")));
        assert!(raw.apply_event(&parse_line("State:D")));
        assert!(raw.apply_event(&parse_line("CC:20")));
        assert!(raw.apply_event(&parse_line("CCEl:16")));
        assert!(raw.apply_event(&parse_line("CCI:15.8;15.9;16.0")));
        assert!(raw.apply_event(&parse_line("T2C:1")));
        assert!(raw.apply_event(&parse_line("FM2:0")));
        assert!(raw.apply_event(&parse_line("D:1")));

        assert_eq!(raw.kit_com, Some(true));
        assert_eq!(raw.initialized, Some(true));
        assert_eq!(raw.reference.as_deref(), Some("058001"));
        assert_eq!(raw.side, Some(1));
        assert_eq!(raw.dip1.as_deref(), Some("1"));
        assert_eq!(raw.state.as_deref(), Some("D"));
        assert_eq!(raw.cc, Some(20));
        assert_eq!(raw.cc_el, Some(16));
        assert_eq!(raw.cci, Some(15.8));
        assert_eq!(raw.cci_l2, Some(15.9));
        assert_eq!(raw.cci_l3, Some(16.0));
        assert_eq!(raw.cci_updates_count, 1);
        assert_eq!(raw.t2c, Some(true));
        assert_eq!(raw.fm2, Some(false));
        assert_eq!(raw.d, Some(true));

        let snap = DriverSnapshot {
            seq: 1,
            atmega: raw,
            usb: UsbLinkState::default(),
        };
        let json_val = snap.to_json();
        assert_eq!(json_val["atmega"]["reference"], "058001");
        assert_eq!(json_val["atmega"]["state"], "D");
        assert_eq!(json_val["atmega"]["cc_el"], 16);
        assert_eq!(json_val["usb"]["status"], "usb_disconnected");
    }
}
