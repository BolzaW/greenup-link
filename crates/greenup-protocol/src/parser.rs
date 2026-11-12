//! Parser for incoming ASCII frames emitted by the ATmega2560 power board (RX).
//!
//! Converts a raw line from the serial link into a structured [`ProtocolEvent`]
//! according to `docs/LEGRAND_SERIAL_PROTOCOL.md`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProtocolEvent {
    // --- 1. System Identification & Boot Handshake ---
    /// `RaspberryPi?` – Boot / periodic supervisor probe from ATmega.
    Ping,
    /// `pong` – Response to `ping`.
    Pong,
    /// `KitCom:0` / `KitCom:1` – Supervisor presence flag in ATmega SRAM.
    KitCom(bool),
    /// `Initialized:0` / `Initialized:1` – Boot autotests completion state.
    Initialized(bool),
    /// `SoftwareVersion:<ver>`
    SoftwareVersion(String),
    /// `HardwareVersion:<ver>`
    HardwareVersion(String),
    /// `SerialNumber:<sn>`
    SerialNumber(String),
    /// `Reference:<ref>`
    Reference(String),
    /// `WeekYearProduction:<wy>`
    WeekYearProduction(String),
    /// `Side:<1/2>`
    Side(String),
    /// `DIP1:<val>` .. `DIP4:<val>` – Physical DIP switch state (`index` in `1..=4`).
    DipSwitch { index: u8, value: String },

    // --- 2. Charge Telemetry, State Machine & Hardware Sensors ---
    /// `State:<A..Z>`
    StateChange(String),
    /// `E:<0000..0015>`
    ErrorChange(String),
    /// `CP:<volts>` – Control Pilot peak voltage (`12`, `9`, `6`, `3`, `0`).
    CpVoltage(u32),
    /// `PP:<amps>` – Proximity Pilot sensed cable ampacity.
    PpCurrent(u32),
    /// `CCCa:<amps>` – Cable Current Capacity.
    CableCurrentCapacity(u32),
    /// `CCI:<l1>[;<l2>;<l3>]` – Instantaneous RMS phase currents in Amperes.
    InstantaneousCurrent {
        l1: f32,
        l2: Option<f32>,
        l3: Option<f32>,
    },
    /// `FCS:0` / `FCS:1` – Full Charge State (vehicle battery full).
    FullChargeState(bool),
    /// `DOM:0` / `DOM:1` – Domestic Schuko plug presence sensor.
    DomesticPlugPresent(bool),
    /// `SB:0` / `SB:1` – Stop/Start button logical state.
    SbState(bool),
    /// `SBF:0` / `SBF:1` – Spontaneous physical plug/button feedback event.
    SbfState(bool),

    // --- 3. Current & Power Modulation ---
    /// `CC:<amps>` – Consolidated active charging limit enforced on the vehicle.
    LimitAmps(u32),
    /// `CCS:<amps>` – Supervisor / Schuko current limit.
    SupervisorLimitAmps(u32),
    /// `CCEl:<amps>` – Cloud Eliot current limit.
    EliotLimitAmps(u32),
    /// `CCCo:<amps>` – Internal consolidated current setpoint.
    ConsolidatedLimitAmps(u32),
    /// `CCTIC:<amps>` – Dynamic current cap calculated from the Linky TIC.
    TicLimitAmps(u32),
    /// `CCSL1:<amps>` .. `CCSL4:<amps>` – Sub-line current limit (`line` in `1..=4`).
    SubLineLimitAmps { line: u8, amps: u32 },
    /// `CCBLES:<amps>` – Bluetooth Standard current setpoint.
    BleStandardLimitAmps(u32),
    /// `CCBLESh:<amps>` – Bluetooth Shunt current setpoint.
    BleShuntLimitAmps(u32),

    // --- 4. Sockets & Actuator Hardware Control ---
    /// `T2C:0` / `T2C:1` – Primary Type 2 socket enabled state.
    T2CEnabled(bool),
    /// `2PC:0` / `2PC:1` – Secondary Domestic (Schuko) socket enabled state.
    DomesticEnabled(bool),
    /// `T2F:0` / `T2F:1` – Forced charging state on Type 2 socket.
    ForceType2State(bool),
    /// `2PF:0` / `2PF:1` – Forced charging state on Domestic socket.
    ForceDomesticState(bool),
    /// `Unlock:ok*` / `Unlock:nok*` – Emergency cable unlock result.
    UnlockResult(String),

    // --- 5. Planning, Scheduling & Modes ---
    /// `PA:0` / `PA:1` – Planning Authorization state.
    PlanningAuthorization(bool),
    /// `PAFC:0` / `PAFC:1` – Planning Auto Force Charge state.
    PlanningAutoForceCharge(bool),
    /// `D:0` / `D:1` – Derogation state.
    Derogation(bool),
    /// `Dec:<min>` – Remaining deferred start delay in minutes.
    DeferredStartDelay(u32),
    /// `Slp:0` / `Slp:1` (or `SOK:ok*` / `SNOK:ok*`) – Vehicle sleep mode state.
    SleepState(bool),
    /// `FM:<1..6>` – Primary Functioning Mode.
    FmMode(String),
    /// `FM2:0` / `FM2:1` – Auxiliary dry contact / external signal state.
    ExternalSignal(bool),
    /// `FM2F:0` / `FM2F:1` – Spontaneous or post-command notification that `FM2` is forced to `0` or `1`.
    ExternalSignalForced(bool),
    /// `CAU:0` / `CAU:1` – Emergency stop interlock state.
    EmergencyStop(bool),

    // --- 6. Modbus RS485 & Energy Metering ---
    /// `EM:<pulses>` – Cumulative active energy pulse counter.
    EnergyMeterPulses(u32),
    /// `ModbusAddr:<val>` – Station Modbus slave address or `ok*`.
    ModbusAddress(String),
    /// `ModbusEMA:<val>` – External Energy Meter Modbus address or `ok*`.
    ModbusEnergyMeterAddress(String),
    /// `ModbusDLMA:<val>` – Dynamic Load Management (DLM) Modbus address or `ok*`.
    ModbusDlmAddress(String),
    /// `ModbusP:<val>` – Modbus parity (`0`, `1`, `2`, or `ok*`).
    ModbusParity(String),
    /// `ModbusB:<val>` – Modbus baud rate code or `ok*`.
    ModbusBaudRate(String),
    /// `ModbusSB:<val>` – Modbus stop bits count (`1`, `2`, or `ok*`).
    ModbusStopBits(String),

    // --- 7. Access Control & RFID Badges ---
    /// `RFIDE:0` / `RFIDE:1` – RFID badge reader peripheral enabled state.
    RfidEnabled(bool),
    /// `RFIDUM:0` / `RFIDUM:1` – RFID User Management mode.
    RfidUserManagement(bool),
    /// `RFIDA:<val>` – RFID badge authorization status (`0`, `1`, or `ok*`).
    RfidAuthorization(String),
    /// `RFIDId:<id>` – Raw badge UID read by the RFID peripheral.
    RfidId(String),
    /// `RFIDIdLP:<val>` – RFID Local Pass enrolment response (`ok*`).
    RfidLocalPassResult(String),
    /// `OCPPStatus:<status>` / `OCPPPS:<status>` – Standardized OCPP 1.6 status.
    OcppStatus(String),
    /// `OCPPPAC:0` / `OCPPPAC:1` – OCPP Plug and Charge state.
    OcppPlugAndCharge(bool),
    /// `OCPPCTO:<val>` – OCPP Connection Timeout response (`ok*`).
    OcppConnectionTimeoutResult(String),

    // --- 8. Bluetooth Controller (BLE) ---
    /// `BT:0` / `BT:1` – Bluetooth module power state.
    BluetoothState(bool),
    /// `MacAdr:<12-hex>` – Bluetooth MAC address (or TIC diagnostic flags on patched FW).
    MacAddress(String),

    // --- 9. Diagnostic & Test Modes ---
    /// `TICTM:0` / `TICTM:1` – Linky TIC trace mode acknowledgment.
    TicTestMode(bool),
    /// `TICTestB:<baud>` – Linky TIC detected baudrate (`0`, `1200`, `9600`).
    TicTestBaud(String),
    /// `TICTestC:...` – Linky TIC test initialization frame.
    TicTestInit,

    // --- 12. Spontaneous Telemetry & Legacy Simulation Frames ---
    /// `CPh:Mono` / `CPh:Tri` – Detected phase configuration at charge startup.
    ChargePhases(String),
    /// `\WT:...:CT:...:\` or `WT:...` – Session summary ticket emitted at end of charge.
    SessionTicket(String),
    /// `Volt:<v>` – Voltage frame (simulator / external).
    Voltage(f32),
    /// `Ener:<wh>` – Energy frame (simulator / external).
    Energy(f32),
    /// `Freq:<hz>` – Frequency frame (simulator / external).
    Frequency(f32),
    /// `Default:<cmd>` – Unrecognized or malformed command rejected by ATmega.
    CommandNotUnderstood(String),
    /// Unrecognized frame.
    Unknown(String),
}

fn parse_bool_01(val: &str) -> Option<bool> {
    match val.trim() {
        "1" => Some(true),
        "0" => Some(false),
        _ => None,
    }
}

fn parse_u32_stripping_unit(val: &str, unit_suffix: char) -> Option<u32> {
    let cleaned = val.trim().trim_end_matches(unit_suffix).trim();
    cleaned.parse::<u32>().ok()
}

pub fn parse_line(line: &str) -> ProtocolEvent {
    let trimmed = line.trim();

    // 1. System Identification & Boot Handshake
    if trimmed.starts_with("RaspberryPi?") {
        return ProtocolEvent::Ping;
    }
    if trimmed == "pong" {
        return ProtocolEvent::Pong;
    }
    if let Some(val) = trimmed.strip_prefix("KitCom:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::KitCom(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("Initialized:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::Initialized(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("SoftwareVersion:") {
        return ProtocolEvent::SoftwareVersion(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("HardwareVersion:") {
        return ProtocolEvent::HardwareVersion(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("SerialNumber:") {
        return ProtocolEvent::SerialNumber(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("Reference:") {
        return ProtocolEvent::Reference(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("WeekYearProduction:") {
        return ProtocolEvent::WeekYearProduction(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("Side:") {
        return ProtocolEvent::Side(val.trim().to_string());
    }
    for idx in 1..=4u8 {
        let prefix = format!("DIP{}:", idx);
        if let Some(val) = trimmed.strip_prefix(&prefix) {
            return ProtocolEvent::DipSwitch {
                index: idx,
                value: val.trim().to_string(),
            };
        }
    }

    // 2. Charge Telemetry, State Machine & Hardware Sensors
    if let Some(val) = trimmed.strip_prefix("State:") {
        return ProtocolEvent::StateChange(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("E:") {
        return ProtocolEvent::ErrorChange(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("CP:") {
        if let Some(v) = parse_u32_stripping_unit(val, 'V') {
            return ProtocolEvent::CpVoltage(v);
        }
    }
    if let Some(val) = trimmed.strip_prefix("PP:") {
        if let Some(amps) = parse_u32_stripping_unit(val, 'A') {
            return ProtocolEvent::PpCurrent(amps);
        }
    }
    if let Some(val) = trimmed.strip_prefix("CCCa:") {
        if let Some(amps) = parse_u32_stripping_unit(val, 'A') {
            return ProtocolEvent::CableCurrentCapacity(amps);
        }
    }
    if let Some(val_str) = trimmed.strip_prefix("CCI:") {
        let mut parts = val_str.split(';').map(|s| s.trim());
        if let Some(first_str) = parts.next() {
            if let Ok(l1) = first_str.parse::<f32>() {
                let l2 = parts.next().and_then(|s| s.parse::<f32>().ok());
                let l3 = parts.next().and_then(|s| s.parse::<f32>().ok());
                return ProtocolEvent::InstantaneousCurrent { l1, l2, l3 };
            }
        }
        return ProtocolEvent::Unknown(line.to_string());
    }
    if let Some(val) = trimmed.strip_prefix("FCS:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::FullChargeState(b);
        }
        // Fallback if bare FCS: was sent
        return ProtocolEvent::FullChargeState(true);
    }
    if let Some(val) = trimmed.strip_prefix("DOM:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::DomesticPlugPresent(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("SB:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::SbState(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("SBF:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::SbfState(b);
        }
    }

    // 3. Current & Power Modulation
    // Note: check longer CC* prefixes before "CC:"!
    if let Some(val) = trimmed.strip_prefix("CCBLESh:") {
        if let Ok(amps) = val.trim().parse::<u32>() {
            return ProtocolEvent::BleShuntLimitAmps(amps);
        }
    }
    if let Some(val) = trimmed.strip_prefix("CCBLES:") {
        if let Ok(amps) = val.trim().parse::<u32>() {
            return ProtocolEvent::BleStandardLimitAmps(amps);
        }
    }
    for line_idx in 1..=4u8 {
        let prefix = format!("CCSL{}:", line_idx);
        if let Some(val) = trimmed.strip_prefix(&prefix) {
            if let Ok(amps) = val.trim().parse::<u32>() {
                return ProtocolEvent::SubLineLimitAmps {
                    line: line_idx,
                    amps,
                };
            }
        }
    }
    if let Some(val) = trimmed.strip_prefix("CCTIC:") {
        if let Ok(amps) = val.trim().parse::<u32>() {
            return ProtocolEvent::TicLimitAmps(amps);
        }
    }
    if let Some(val) = trimmed.strip_prefix("CCCo:") {
        if let Ok(amps) = val.trim().parse::<u32>() {
            return ProtocolEvent::ConsolidatedLimitAmps(amps);
        }
    }
    if let Some(val) = trimmed.strip_prefix("CCEl:") {
        if let Ok(amps) = val.trim().parse::<u32>() {
            return ProtocolEvent::EliotLimitAmps(amps);
        }
    }
    if let Some(val) = trimmed.strip_prefix("CCS:") {
        if let Ok(amps) = val.trim().parse::<u32>() {
            return ProtocolEvent::SupervisorLimitAmps(amps);
        }
    }
    if let Some(val) = trimmed.strip_prefix("CC:") {
        if let Ok(amps) = val.trim().parse::<u32>() {
            return ProtocolEvent::LimitAmps(amps);
        }
    }

    // 4. Sockets & Actuator Hardware Control
    if let Some(val) = trimmed.strip_prefix("T2C:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::T2CEnabled(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("2PC:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::DomesticEnabled(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("T2F:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::ForceType2State(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("2PF:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::ForceDomesticState(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("Unlock:") {
        return ProtocolEvent::UnlockResult(val.trim().to_string());
    }

    // 5. Planning, Scheduling & Modes
    if let Some(val) = trimmed.strip_prefix("PAFC:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::PlanningAutoForceCharge(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("PA:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::PlanningAuthorization(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("D:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::Derogation(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("Dec:") {
        if let Ok(mins) = val.trim().parse::<u32>() {
            return ProtocolEvent::DeferredStartDelay(mins);
        }
    }
    if let Some(val) = trimmed.strip_prefix("Slp:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::SleepState(b);
        }
    }
    if trimmed.starts_with("SOK:ok") {
        return ProtocolEvent::SleepState(true);
    }
    if trimmed.starts_with("SNOK:ok") {
        return ProtocolEvent::SleepState(false);
    }
    if let Some(val) = trimmed.strip_prefix("FM2:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::ExternalSignal(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("FM2F:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::ExternalSignalForced(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("FM:") {
        return ProtocolEvent::FmMode(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("CAU:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::EmergencyStop(b);
        }
    }

    // 6. Modbus RS485 & Energy Metering
    if let Some(val) = trimmed.strip_prefix("EM:") {
        if let Ok(pulses) = val.trim().parse::<u32>() {
            return ProtocolEvent::EnergyMeterPulses(pulses);
        }
    }
    if let Some(val) = trimmed.strip_prefix("ModbusAddr:") {
        return ProtocolEvent::ModbusAddress(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("ModbusEMA:") {
        return ProtocolEvent::ModbusEnergyMeterAddress(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("ModbusDLMA:") {
        return ProtocolEvent::ModbusDlmAddress(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("ModbusP:") {
        return ProtocolEvent::ModbusParity(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("ModbusB:") {
        return ProtocolEvent::ModbusBaudRate(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("ModbusSB:") {
        return ProtocolEvent::ModbusStopBits(val.trim().to_string());
    }

    // 7. Access Control & RFID Badges
    if let Some(val) = trimmed.strip_prefix("RFIDE:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::RfidEnabled(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("RFIDUM:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::RfidUserManagement(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("RFIDA:") {
        return ProtocolEvent::RfidAuthorization(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("RFIDIdLP:") {
        return ProtocolEvent::RfidLocalPassResult(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("RFIDId:") {
        return ProtocolEvent::RfidId(val.trim().to_string());
    }
    if let Some(val) = trimmed
        .strip_prefix("OCPPStatus:")
        .or_else(|| trimmed.strip_prefix("OCPPPS:"))
    {
        return ProtocolEvent::OcppStatus(val.trim().to_string());
    }
    if let Some(val) = trimmed.strip_prefix("OCPPPAC:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::OcppPlugAndCharge(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("OCPPCTO:") {
        return ProtocolEvent::OcppConnectionTimeoutResult(val.trim().to_string());
    }

    // 8. Bluetooth Controller (BLE)
    if let Some(val) = trimmed.strip_prefix("BT:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::BluetoothState(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("MacAdr:") {
        return ProtocolEvent::MacAddress(val.trim().to_string());
    }

    // 9. Diagnostic & Test Modes
    if let Some(val) = trimmed.strip_prefix("TICTM:") {
        if let Some(b) = parse_bool_01(val) {
            return ProtocolEvent::TicTestMode(b);
        }
    }
    if let Some(val) = trimmed.strip_prefix("TICTestB:") {
        return ProtocolEvent::TicTestBaud(val.trim().to_string());
    }
    if trimmed.starts_with("TICTestC:") {
        return ProtocolEvent::TicTestInit;
    }

    // 12. Spontaneous Telemetry & Simulator Frames
    if let Some(val) = trimmed
        .strip_prefix("CPh:")
        .or_else(|| trimmed.strip_prefix("Cph:"))
    {
        return ProtocolEvent::ChargePhases(val.trim().to_string());
    }
    let clean_ticket = trimmed.trim_matches('\\');
    if clean_ticket.starts_with("WT:") {
        return ProtocolEvent::SessionTicket(clean_ticket.to_string());
    }
    if let Some(val) = trimmed.strip_prefix("Volt:") {
        if let Ok(v) = val.trim().parse::<f32>() {
            return ProtocolEvent::Voltage(v);
        }
    }
    if let Some(val) = trimmed.strip_prefix("Ener:") {
        if let Ok(e) = val.trim().parse::<f32>() {
            return ProtocolEvent::Energy(e);
        }
    }
    if let Some(val) = trimmed.strip_prefix("Freq:") {
        if let Ok(f) = val.trim().parse::<f32>() {
            return ProtocolEvent::Frequency(f);
        }
    }
    if let Some(cmd) = trimmed.strip_prefix("Default:") {
        return ProtocolEvent::CommandNotUnderstood(cmd.trim().to_string());
    }

    ProtocolEvent::Unknown(line.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_identity_and_boot_frames() {
        assert_eq!(parse_line("RaspberryPi?"), ProtocolEvent::Ping);
        assert_eq!(parse_line("pong\r"), ProtocolEvent::Pong);
        assert_eq!(parse_line("KitCom:1"), ProtocolEvent::KitCom(true));
        assert_eq!(parse_line("KitCom:0"), ProtocolEvent::KitCom(false));
        assert_eq!(parse_line("Initialized:1"), ProtocolEvent::Initialized(true));
        assert_eq!(parse_line("Initialized:0"), ProtocolEvent::Initialized(false));
        assert_eq!(
            parse_line("MacAdr:001A7DDA7113"),
            ProtocolEvent::MacAddress("001A7DDA7113".into())
        );
        assert_eq!(parse_line("Side:49"), ProtocolEvent::Side("49".into()));
        assert_eq!(
            parse_line("DIP1:1"),
            ProtocolEvent::DipSwitch {
                index: 1,
                value: "1".into()
            }
        );
        assert_eq!(
            parse_line("DIP4:0"),
            ProtocolEvent::DipSwitch {
                index: 4,
                value: "0".into()
            }
        );
    }

    #[test]
    fn parses_sensors_and_phase_currents() {
        assert_eq!(parse_line("CP:9"), ProtocolEvent::CpVoltage(9));
        assert_eq!(parse_line("CP:12V"), ProtocolEvent::CpVoltage(12));
        assert_eq!(parse_line("PP:20"), ProtocolEvent::PpCurrent(20));
        assert_eq!(parse_line("PP:32A"), ProtocolEvent::PpCurrent(32));
        assert_eq!(
            parse_line("CCCa:32"),
            ProtocolEvent::CableCurrentCapacity(32)
        );
        assert_eq!(
            parse_line("CCI:24.48"),
            ProtocolEvent::InstantaneousCurrent {
                l1: 24.48,
                l2: None,
                l3: None
            }
        );
        assert_eq!(
            parse_line("CCI:15.98;16.02;15.95"),
            ProtocolEvent::InstantaneousCurrent {
                l1: 15.98,
                l2: Some(16.02),
                l3: Some(15.95)
            }
        );
        assert_eq!(parse_line("FCS:0"), ProtocolEvent::FullChargeState(false));
        assert_eq!(parse_line("FCS:1"), ProtocolEvent::FullChargeState(true));
        assert_eq!(
            parse_line("DOM:1"),
            ProtocolEvent::DomesticPlugPresent(true)
        );
        assert_eq!(
            parse_line("DOM:0"),
            ProtocolEvent::DomesticPlugPresent(false)
        );
    }

    #[test]
    fn parses_all_current_limit_layers() {
        assert_eq!(parse_line("CC:16"), ProtocolEvent::LimitAmps(16));
        assert_eq!(parse_line("CCS:20"), ProtocolEvent::SupervisorLimitAmps(20));
        assert_eq!(parse_line("CCEl:16"), ProtocolEvent::EliotLimitAmps(16));
        assert_eq!(
            parse_line("CCCo:16"),
            ProtocolEvent::ConsolidatedLimitAmps(16)
        );
        assert_eq!(parse_line("CCTIC:32"), ProtocolEvent::TicLimitAmps(32));
        assert_eq!(
            parse_line("CCSL2:14"),
            ProtocolEvent::SubLineLimitAmps { line: 2, amps: 14 }
        );
        assert_eq!(
            parse_line("CCBLES:20"),
            ProtocolEvent::BleStandardLimitAmps(20)
        );
        assert_eq!(
            parse_line("CCBLESh:10"),
            ProtocolEvent::BleShuntLimitAmps(10)
        );
    }

    #[test]
    fn parses_sockets_planning_modbus_and_rfid() {
        assert_eq!(parse_line("T2C:1"), ProtocolEvent::T2CEnabled(true));
        assert_eq!(parse_line("2PC:0"), ProtocolEvent::DomesticEnabled(false));
        assert_eq!(parse_line("T2F:1"), ProtocolEvent::ForceType2State(true));
        assert_eq!(parse_line("2PF:0"), ProtocolEvent::ForceDomesticState(false));
        assert_eq!(
            parse_line("Unlock:ok*"),
            ProtocolEvent::UnlockResult("ok*".into())
        );
        assert_eq!(
            parse_line("PA:1"),
            ProtocolEvent::PlanningAuthorization(true)
        );
        assert_eq!(
            parse_line("PAFC:0"),
            ProtocolEvent::PlanningAutoForceCharge(false)
        );
        assert_eq!(parse_line("Dec:120"), ProtocolEvent::DeferredStartDelay(120));
        assert_eq!(parse_line("Slp:1"), ProtocolEvent::SleepState(true));
        assert_eq!(parse_line("SOK:ok*"), ProtocolEvent::SleepState(true));
        assert_eq!(parse_line("SNOK:ok*"), ProtocolEvent::SleepState(false));
        assert_eq!(parse_line("CAU:0"), ProtocolEvent::EmergencyStop(false));
        assert_eq!(
            parse_line("EM:4520"),
            ProtocolEvent::EnergyMeterPulses(4520)
        );
        assert_eq!(
            parse_line("ModbusAddr:1"),
            ProtocolEvent::ModbusAddress("1".into())
        );
        assert_eq!(parse_line("RFIDE:1"), ProtocolEvent::RfidEnabled(true));
        assert_eq!(
            parse_line("RFIDUM:0"),
            ProtocolEvent::RfidUserManagement(false)
        );
        assert_eq!(
            parse_line("OCPPStatus:Preparing-T2S"),
            ProtocolEvent::OcppStatus("Preparing-T2S".into())
        );
        assert_eq!(
            parse_line("OCPPPAC:1"),
            ProtocolEvent::OcppPlugAndCharge(true)
        );
    }

    #[test]
    fn parses_spontaneous_tickets_and_phases() {
        assert_eq!(
            parse_line("CPh:Mono"),
            ProtocolEvent::ChargePhases("Mono".into())
        );
        assert_eq!(
            parse_line("Cph:Tri"),
            ProtocolEvent::ChargePhases("Tri".into())
        );
        assert_eq!(
            parse_line("FM2F:1"),
            ProtocolEvent::ExternalSignalForced(true)
        );
        assert_eq!(
            parse_line("FM2F:0"),
            ProtocolEvent::ExternalSignalForced(false)
        );
        assert_eq!(
            parse_line("\\WT:00h01:CT:01h20:EVplug:12000:\\"),
            ProtocolEvent::SessionTicket("WT:00h01:CT:01h20:EVplug:12000:".into())
        );
    }
}
