//! Commands issued to the power board (TX).
//!
//! Each variant of [`Command`] corresponds to an ASCII frame documented in
//! `docs/LEGRAND_SERIAL_PROTOCOL.md`. The [`Command::encode`] method produces the
//! frame ready to be written to the serial link (terminated by `\r`).
//!
//! Note: Factory EEPROM calibration commands (Section 10), service/prodMode test commands
//! (Section 11), and the weekly `Calendar?` command are intentionally excluded.
//!
//! This layer is intentionally pure: no I/O, no state.

use std::fmt;

/// Frame terminator expected by the ATmega (Carriage Return, 0x0D).
pub const FRAME_TERMINATOR: &str = "\r";

/// Minimum current accepted by the board (below this, the board goes into fault state).
pub const MIN_CURRENT_AMPS: u8 = 7;
/// Maximum current accepted by the board.
pub const MAX_CURRENT_AMPS: u8 = 32;

/// Main functioning modes of the ATmega (`FM:X`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctioningMode {
    /// `FM:1` – Permanent direct charge (mode used by Green'Up Link).
    DirectCharge = 1,
    /// `FM:2` – Controlled by dry contact (Off-Peak / Peak hours).
    RemoteControl = 2,
    /// `FM:3` – Smart meter (TIC). Not implemented in firmware 18.04.
    SmartMeter = 3,
    /// `FM:4` – Internal time programming.
    Programming = 4,
    /// `FM:5` – Modbus control (DLM).
    Modbus = 5,
    /// `FM:6` – OCPP supervision (heavily modifies internal behavior).
    Ocpp = 6,
}

impl FunctioningMode {
    /// Numeric code sent in the `FM:X` frame.
    pub fn code(self) -> u8 {
        self as u8
    }

    /// Decodes the value received in an `FM:X` frame.
    pub fn from_code(code: &str) -> Option<Self> {
        match code.trim() {
            "1" => Some(Self::DirectCharge),
            "2" => Some(Self::RemoteControl),
            "3" => Some(Self::SmartMeter),
            "4" => Some(Self::Programming),
            "5" => Some(Self::Modbus),
            "6" => Some(Self::Ocpp),
            _ => None,
        }
    }
}

/// Errors when building a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandError {
    /// The requested current is outside the range [`MIN_CURRENT_AMPS`]..=[`MAX_CURRENT_AMPS`].
    CurrentOutOfRange(u8),
    /// The requested sub-line index is outside the range `1..=4`.
    SubLineOutOfRange(u8),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::CurrentOutOfRange(a) => write!(
                f,
                "current {}A out of bounds ({}..={}A)",
                a, MIN_CURRENT_AMPS, MAX_CURRENT_AMPS
            ),
            CommandError::SubLineOutOfRange(idx) => {
                write!(f, "sub-line index {} out of bounds (1..=4)", idx)
            }
        }
    }
}

impl std::error::Error for CommandError {}

/// All operational commands that can be sent to the power board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    // --- 1. System Identification & Boot Handshake ---
    /// `RaspberryPiModeOK` – Announces that the Pi supervisor is taking control.
    RaspberryPiModeOk,
    /// `KitCom?` – Queries whether supervisor handshake is active (`KitCom:0` or `1`).
    GetKitCom,
    /// `SoftwareVersion?`
    GetSoftwareVersion,
    /// `HardwareVersion?`
    GetHardwareVersion,
    /// `SerialNumber?`
    GetSerialNumber,
    /// `Reference?`
    GetReference,
    /// `WeekYearProduction?`
    GetWeekYearProduction,
    /// `Side?` – Active charge point side (`1` or `2`).
    GetSide,
    /// `Initialized?` – Queries whether power board boot autotests are completed (`0` or `1`).
    GetInitialized,
    /// `Reset` – Reboots the power board.
    Reset,
    /// `ping` – Basic keepalive probe (`pong`).
    Ping,

    // --- 2. Charge Telemetry, State Machine & Hardware Sensors ---
    /// `State?` – Main charging FSM state (`State:A`..`Z`).
    GetState,
    /// `E?` – System error code (`E:0000`..`E:0015`).
    GetErrors,
    /// `CP?` – Control Pilot peak voltage (`12`, `9`, `6`, `3`, `0`).
    GetCpVoltage,
    /// `PP?` – Proximity Pilot sensed cable ampacity.
    GetPpCurrent,
    /// `CCCa?` – Cable Current Capacity.
    GetCableCurrentCapacity,
    /// `CCI?` – Instantaneous RMS phase currents (`CCI:L1` or `CCI:L1;L2;L3`).
    GetInstantaneousCurrent,
    /// `FCS?` – Full Charge State (`FCS:0` or `FCS:1`).
    GetFullChargeState,
    /// `DOM?` – Domestic Schuko socket plug presence sensor (`DOM:0` or `DOM:1`).
    GetDomesticPlugPresent,
    /// `SB?` – Front Stop/Start button logical state (`SB:0` or `SB:1`).
    GetSbState,
    /// `SBOK` / `SBNOK` – Simulates front panel button release (`SBOK` = resume) or press (`SBNOK` = pause).
    SetStartButton(bool),

    // --- 3. Current & Power Modulation ---
    /// `CC?` – Queries consolidated active charging limit enforced on the vehicle.
    GetCurrentLimit,
    /// `CC:XX` – Direct current override on the vehicle CP PWM.
    SetDirectCurrentLimit(u8),
    /// `CCS?` – Queries Supervisor Current limit (`CCS:`).
    GetSupervisorCurrentLimit,
    /// `CCS:XX` – Sets Supervisor / Schuko current limit.
    SetSchukoCurrentLimit(u8),
    /// `CCEl?` – Queries Cloud Eliot Current limit (`CCEl:`).
    GetEliotCurrentLimit,
    /// `CCEl:XX` – Sets Cloud Eliot Current limit (primary method used by Green'Up Link).
    SetCurrentLimit(u8),
    /// `CCCo?` – Queries internal Consolidated Current setpoint (`CCCo:`).
    GetConsolidatedCurrentLimit,
    /// `CCTIC?` – Queries dynamic current cap calculated from the Linky TIC (`CCTIC:`).
    GetTicCurrentLimit,
    /// `CCSL1?`..`CCSL4?` – Queries sub-line current limit for cluster line `1..=4`.
    GetSubLineCurrentLimit(u8),
    /// `CCBLES?` – Queries Bluetooth Standard current setpoint (`CCBLES:`).
    GetBleStandardCurrentLimit,
    /// `CCBLESh?` – Queries Bluetooth Shunt current setpoint (`CCBLESh:`).
    GetBleShuntCurrentLimit,

    // --- 4. Sockets & Actuator Hardware Control ---
    /// `T2C?` – Queries whether Type 2 socket is enabled (`T2C:0` or `1`).
    GetT2CEnabled,
    /// `T2COK` / `T2CNOK` – Authorizes/blocks Type 2 charging.
    AuthorizeType2(bool),
    /// `2PC?` – Queries whether domestic (Schuko) socket is enabled (`2PC:0` or `1`).
    GetDomesticEnabled,
    /// `2PCOK` / `2PCNOK` – Authorizes/blocks domestic socket charging.
    AuthorizeDomestic(bool),
    /// `T2F?` – Queries whether charging is forced on Type 2 socket (`T2F:0` or `1`).
    GetForceType2,
    /// `T2FOK` / `T2FNOK` – Forces Type 2 charging.
    ForceType2(bool),
    /// `2PF?` – Queries whether charging is forced on domestic socket (`2PF:0` or `1`).
    GetForceDomestic,
    /// `2PFOK` / `2PFNOK` – Forces domestic socket charging.
    ForceDomestic(bool),
    /// `Unlock` – Physically unlocks the Type 2 cable.
    Unlock,

    // --- 5. Planning, Scheduling & Modes (excluding Calendar?) ---
    /// `PA?` – Queries Planning Authorization (`PA:0` or `1`).
    GetPlanningAuthorization,
    /// `PAOK` / `PANOK` – Enables/disables adherence to planned charging schedule.
    SetPlanningAuthorization(bool),
    /// `PAFC?` – Queries Planning Auto Force Charge (`PAFC:0` or `1`).
    GetPlanningAutoForceCharge,
    /// `PAFCOK` / `PAFCNOK` – Enables/disables automatic forced charging over schedule windows.
    SetPlanningAutoForceCharge(bool),
    /// `D?` – Queries Derogation state (`D:0` or `1`).
    GetDerogation,
    /// `DOK` / `DNOK` – Enables/disables manual schedule derogation.
    SetDerogation(bool),
    /// `Dec?` – Queries remaining deferred start delay in minutes (`Dec:<min>`).
    GetDeferredStartDelay,
    /// `Dec:<min>` – Sets deferred start delay in minutes.
    SetDeferredStartDelay(u16),
    /// `Slp?` – Queries whether station is in vehicle sleep mode (`Slp:0` or `1`).
    GetSleep,
    /// `SOK` / `SNOK` – Enters (`SOK`) or exits (`SNOK`) vehicle sleep mode.
    SetSleep(bool),
    /// `FM?` – Queries primary Functioning Mode (`FM:1`..`6`).
    GetFunctioningMode,
    /// `FM:X` – Sets primary Functioning Mode.
    SetFunctioningMode(FunctioningMode),
    /// `FM2?` – Queries auxiliary dry contact / Eco-Start state (`FM2:0` or `1`).
    GetExternalSignal,
    /// `FM2:1` / `FM2:0` – Simulates external dry contact state (`1` = blocked, `0` = authorized).
    SetExternalSignal(bool),
    /// `CAU?` – Queries Emergency Stop interlock state (`CAU:0` or `1`).
    GetEmergencyStop,
    /// `CAUOK` / `CAUNOK` – Toggles emergency stop safety lockout state.
    SetEmergencyStop(bool),

    // --- 6. Modbus RS485 & Energy Metering ---
    /// `EM?` – Queries cumulative Energy Meter pulse count (`EM:<pulses>`).
    GetEnergyMeterPulses,
    /// `ModbusAddr?` – Queries station's Modbus RS485 slave address.
    GetModbusAddress,
    /// `ModbusAddr:<addr>` – Sets station's Modbus RS485 slave address.
    SetModbusAddress(u8),
    /// `ModbusEMA?` – Queries external Energy Meter Modbus address.
    GetModbusEnergyMeterAddress,
    /// `ModbusEMA:<addr>` – Sets external Energy Meter Modbus address.
    SetModbusEnergyMeterAddress(u8),
    /// `ModbusDLMA?` – Queries Dynamic Load Management (DLM) Modbus address.
    GetModbusDlmAddress,
    /// `ModbusDLMA:<addr>` – Sets Dynamic Load Management (DLM) Modbus address.
    SetModbusDlmAddress(u8),
    /// `ModbusP?` – Queries Modbus parity (`0` = None, `1` = Even, `2` = Odd).
    GetModbusParity,
    /// `ModbusP:<0/1/2>` – Sets Modbus parity.
    SetModbusParity(u8),
    /// `ModbusB?` – Queries Modbus baud rate code.
    GetModbusBaudRate,
    /// `ModbusB:<baud>` – Sets Modbus baud rate code.
    SetModbusBaudRate(u8),
    /// `ModbusSB?` – Queries Modbus stop bits count (`1` or `2`).
    GetModbusStopBits,
    /// `ModbusSB:<1/2>` – Sets Modbus stop bits count.
    SetModbusStopBits(u8),

    // --- 7. Access Control & RFID Badges ---
    /// `RFIDE?` – Queries whether RFID badge reader peripheral is enabled (`RFIDE:0` or `1`).
    GetRfidEnabled,
    /// `RFIDEOK` / `RFIDENOK` – Enables/disables RFID badge reader peripheral.
    SetRfidEnabled(bool),
    /// `RFIDUM?` – Queries RFID User Management mode (`RFIDUM:0` or `1`).
    GetRfidUserManagement,
    /// `RFIDA?` – Queries whether badge authorization is currently granted (`RFIDA:0` or `1`).
    GetRfidAuthorization,
    /// `RFIDA:<tag>` – Transmits RFID authorization status/tag.
    SendRfidAuthorization(String),
    /// `RFIDId:<id>` – Transmits badge identifier.
    SendRfidId(String),
    /// `RFIDIdLP:<id>` – Enrols a permanent local pass RFID badge.
    EnrollRfidLocalPass(String),
    /// `OCPPPS?` – Queries OCPP Power Status (`OCPPStatus:`).
    GetOcppParameters,
    /// `OCPPPACOK` / `OCPPPACNOK` – Enables/disables OCPP Plug & Charge.
    SetPlugAndCharge(bool),
    /// `OCPPCTO:<sec>` – Configures OCPP connection timeout in seconds.
    SetOcppConnectionTimeout(u32),

    // --- 8. Bluetooth Controller (BLE) ---
    /// `BT?` – Queries Bluetooth module power state (`BT:0` or `1`).
    GetBluetooth,
    /// `BTOK` / `BTNOK` – Powers on (`BTOK`) or off (`BTNOK`) the Bluetooth module.
    SetBluetooth(bool),
    /// `MacAdr?` – Queries 12-hex Bluetooth MAC address (or Linky TIC diagnostic flags on patched FW).
    GetMacAddress,

    // --- 9. Diagnostic & Test Modes ---
    /// `TICTM:1` / `TICTM:0` – Enables/disables Linky TIC trace/detection mode.
    SetTicTestMode(bool),

    // --- 10. Raw Debug ---
    /// Sending a raw command without expecting an acknowledgment (debug API).
    Raw(String),
}

fn ok_nok(prefix: &str, enabled: bool) -> String {
    format!("{}{}", prefix, if enabled { "OK" } else { "NOK" })
}

impl Command {
    /// Builds a `CCEl:XX` command while validating the current range.
    pub fn set_current_limit(amps: u8) -> Result<Self, CommandError> {
        Self::validate_current(amps).map(Command::SetCurrentLimit)
    }

    /// Builds a direct `CC:XX` command while validating the current range.
    pub fn set_direct_current_limit(amps: u8) -> Result<Self, CommandError> {
        Self::validate_current(amps).map(Command::SetDirectCurrentLimit)
    }

    /// Builds a `CCS:XX` command while validating the current range.
    pub fn set_schuko_current_limit(amps: u8) -> Result<Self, CommandError> {
        Self::validate_current(amps).map(Command::SetSchukoCurrentLimit)
    }

    /// Builds a `CCSL<1..4>?` query while validating the sub-line index (`1..=4`).
    pub fn get_sub_line_current_limit(line: u8) -> Result<Self, CommandError> {
        if (1..=4).contains(&line) {
            Ok(Command::GetSubLineCurrentLimit(line))
        } else {
            Err(CommandError::SubLineOutOfRange(line))
        }
    }

    fn validate_current(amps: u8) -> Result<u8, CommandError> {
        if (MIN_CURRENT_AMPS..=MAX_CURRENT_AMPS).contains(&amps) {
            Ok(amps)
        } else {
            Err(CommandError::CurrentOutOfRange(amps))
        }
    }

    /// ASCII frame without terminator (useful for logs).
    pub fn as_frame(&self) -> String {
        match self {
            // 1. System Identification & Boot Handshake
            Command::RaspberryPiModeOk => "RaspberryPiModeOK".into(),
            Command::GetKitCom => "KitCom?".into(),
            Command::GetSoftwareVersion => "SoftwareVersion?".into(),
            Command::GetHardwareVersion => "HardwareVersion?".into(),
            Command::GetSerialNumber => "SerialNumber?".into(),
            Command::GetReference => "Reference?".into(),
            Command::GetWeekYearProduction => "WeekYearProduction?".into(),
            Command::GetSide => "Side?".into(),
            Command::GetInitialized => "Initialized?".into(),
            Command::Reset => "Reset".into(),
            Command::Ping => "ping".into(),

            // 2. Charge Telemetry, State Machine & Hardware Sensors
            Command::GetState => "State?".into(),
            Command::GetErrors => "E?".into(),
            Command::GetCpVoltage => "CP?".into(),
            Command::GetPpCurrent => "PP?".into(),
            Command::GetCableCurrentCapacity => "CCCa?".into(),
            Command::GetInstantaneousCurrent => "CCI?".into(),
            Command::GetFullChargeState => "FCS?".into(),
            Command::GetDomesticPlugPresent => "DOM?".into(),
            Command::GetSbState => "SB?".into(),
            Command::SetStartButton(on) => ok_nok("SB", *on),

            // 3. Current & Power Modulation
            Command::GetCurrentLimit => "CC?".into(),
            Command::SetDirectCurrentLimit(a) => format!("CC:{:02}", a),
            Command::GetSupervisorCurrentLimit => "CCS?".into(),
            Command::SetSchukoCurrentLimit(a) => format!("CCS:{:02}", a),
            Command::GetEliotCurrentLimit => "CCEl?".into(),
            Command::SetCurrentLimit(a) => format!("CCEl:{:02}", a),
            Command::GetConsolidatedCurrentLimit => "CCCo?".into(),
            Command::GetTicCurrentLimit => "CCTIC?".into(),
            Command::GetSubLineCurrentLimit(line) => format!("CCSL{}?", line.clamp(&1, &4)),
            Command::GetBleStandardCurrentLimit => "CCBLES?".into(),
            Command::GetBleShuntCurrentLimit => "CCBLESh?".into(),

            // 4. Sockets & Actuator Hardware Control
            Command::GetT2CEnabled => "T2C?".into(),
            Command::AuthorizeType2(on) => ok_nok("T2C", *on),
            Command::GetDomesticEnabled => "2PC?".into(),
            Command::AuthorizeDomestic(on) => ok_nok("2PC", *on),
            Command::GetForceType2 => "T2F?".into(),
            Command::ForceType2(on) => ok_nok("T2F", *on),
            Command::GetForceDomestic => "2PF?".into(),
            Command::ForceDomestic(on) => ok_nok("2PF", *on),
            Command::Unlock => "Unlock".into(),

            // 5. Planning, Scheduling & Modes
            Command::GetPlanningAuthorization => "PA?".into(),
            Command::SetPlanningAuthorization(on) => ok_nok("PA", *on),
            Command::GetPlanningAutoForceCharge => "PAFC?".into(),
            Command::SetPlanningAutoForceCharge(on) => ok_nok("PAFC", *on),
            Command::GetDerogation => "D?".into(),
            Command::SetDerogation(accept) => if *accept { "DOK".into() } else { "DNOK".into() },
            Command::GetDeferredStartDelay => "Dec?".into(),
            Command::SetDeferredStartDelay(mins) => format!("Dec:{}", mins),
            Command::GetSleep => "Slp?".into(),
            Command::SetSleep(on) => ok_nok("S", *on),
            Command::GetFunctioningMode => "FM?".into(),
            Command::SetFunctioningMode(m) => format!("FM:{}", m.code()),
            Command::GetExternalSignal => "FM2?".into(),
            Command::SetExternalSignal(on) => format!("FM2:{}", *on as u8),
            Command::GetEmergencyStop => "CAU?".into(),
            Command::SetEmergencyStop(on) => ok_nok("CAU", *on),

            // 6. Modbus RS485 & Energy Metering
            Command::GetEnergyMeterPulses => "EM?".into(),
            Command::GetModbusAddress => "ModbusAddr?".into(),
            Command::SetModbusAddress(addr) => format!("ModbusAddr:{}", addr),
            Command::GetModbusEnergyMeterAddress => "ModbusEMA?".into(),
            Command::SetModbusEnergyMeterAddress(addr) => format!("ModbusEMA:{}", addr),
            Command::GetModbusDlmAddress => "ModbusDLMA?".into(),
            Command::SetModbusDlmAddress(addr) => format!("ModbusDLMA:{}", addr),
            Command::GetModbusParity => "ModbusP?".into(),
            Command::SetModbusParity(p) => format!("ModbusP:{}", p),
            Command::GetModbusBaudRate => "ModbusB?".into(),
            Command::SetModbusBaudRate(b) => format!("ModbusB:{}", b),
            Command::GetModbusStopBits => "ModbusSB?".into(),
            Command::SetModbusStopBits(sb) => format!("ModbusSB:{}", sb),

            // 7. Access Control & RFID Badges
            Command::GetRfidEnabled => "RFIDE?".into(),
            Command::SetRfidEnabled(on) => ok_nok("RFIDE", *on),
            Command::GetRfidUserManagement => "RFIDUM?".into(),
            Command::GetRfidAuthorization => "RFIDA?".into(),
            Command::SendRfidAuthorization(s) => format!("RFIDA:{}", s),
            Command::SendRfidId(id) => format!("RFIDId:{}", id),
            Command::EnrollRfidLocalPass(id) => format!("RFIDIdLP:{}", id),
            Command::GetOcppParameters => "OCPPPS?".into(),
            Command::SetPlugAndCharge(on) => ok_nok("OCPPPAC", *on),
            Command::SetOcppConnectionTimeout(t) => format!("OCPPCTO:{}", t),

            // 8. Bluetooth Controller (BLE)
            Command::GetBluetooth => "BT?".into(),
            Command::SetBluetooth(on) => ok_nok("BT", *on),
            Command::GetMacAddress => "MacAdr?".into(),

            // 9. Diagnostic & Test Modes
            Command::SetTicTestMode(on) => format!("TICTM:{}", *on as u8),

            // 10. Raw Debug
            Command::Raw(s) => s.clone(),
        }
    }

    /// Complete frame ready to be written to the serial link (with `\r`).
    pub fn encode(&self) -> String {
        format!("{}{}", self.as_frame(), FRAME_TERMINATOR)
    }

    /// Determines the expected prefix in response to acknowledge this command.
    /// If `None`, the command does not require an acknowledgment or is "fire-and-forget" (e.g. Raw).
    pub fn expected_rx_prefix(&self) -> Option<&'static str> {
        match self {
            // 1. System Identification & Boot Handshake
            Command::RaspberryPiModeOk => Some("DIP1:"),
            Command::GetKitCom => Some("KitCom:"),
            Command::GetSoftwareVersion => Some("SoftwareVersion:"),
            Command::GetHardwareVersion => Some("HardwareVersion:"),
            Command::GetSerialNumber => Some("SerialNumber:"),
            Command::GetReference => Some("Reference:"),
            Command::GetWeekYearProduction => Some("WeekYearProduction:"),
            Command::GetSide => Some("Side:"),
            Command::GetInitialized => Some("Initialized:"),
            Command::Reset => Some("State:"), // Reset returns State:X / State:A after reboot
            Command::Ping => Some("pong"),

            // 2. Charge Telemetry, State Machine & Hardware Sensors
            Command::GetState => Some("State:"),
            Command::GetErrors => Some("E:"),
            Command::GetCpVoltage => Some("CP:"),
            Command::GetPpCurrent => Some("PP:"),
            Command::GetCableCurrentCapacity => Some("CCCa:"),
            Command::GetInstantaneousCurrent => Some("CCI:"),
            Command::GetFullChargeState => Some("FCS:"),
            Command::GetDomesticPlugPresent => Some("DOM:"),
            Command::GetSbState => Some("SB:"),
            Command::SetStartButton(_) => Some("SB:"),

            // 3. Current & Power Modulation
            Command::GetCurrentLimit => Some("CC:"),
            Command::SetDirectCurrentLimit(_) => Some("CC:"),
            Command::GetSupervisorCurrentLimit => Some("CCS:"),
            Command::SetSchukoCurrentLimit(_) => Some("CCS:"),
            Command::GetEliotCurrentLimit => Some("CCEl:"),
            Command::SetCurrentLimit(_) => Some("CCEl:"),
            Command::GetConsolidatedCurrentLimit => Some("CCCo:"),
            Command::GetTicCurrentLimit => Some("CCTIC:"),
            Command::GetSubLineCurrentLimit(1) => Some("CCSL1:"),
            Command::GetSubLineCurrentLimit(2) => Some("CCSL2:"),
            Command::GetSubLineCurrentLimit(3) => Some("CCSL3:"),
            Command::GetSubLineCurrentLimit(_) => Some("CCSL4:"),
            Command::GetBleStandardCurrentLimit => Some("CCBLES:"),
            Command::GetBleShuntCurrentLimit => Some("CCBLESh:"),

            // 4. Sockets & Actuator Hardware Control
            Command::GetT2CEnabled => Some("T2C:"),
            Command::AuthorizeType2(true) => Some("T2C:1"),
            Command::AuthorizeType2(false) => Some("T2C:0"),
            Command::GetDomesticEnabled => Some("2PC:"),
            Command::AuthorizeDomestic(true) => Some("2PC:1"),
            Command::AuthorizeDomestic(false) => Some("2PC:0"),
            Command::GetForceType2 => Some("T2F:"),
            Command::ForceType2(true) => Some("T2F:1"),
            Command::ForceType2(false) => Some("T2F:0"),
            Command::GetForceDomestic => Some("2PF:"),
            Command::ForceDomestic(true) => Some("2PF:1"),
            Command::ForceDomestic(false) => Some("2PF:0"),
            Command::Unlock => Some("Unlock"),

            // 5. Planning, Scheduling & Modes
            Command::GetPlanningAuthorization => Some("PA:"),
            Command::SetPlanningAuthorization(true) => Some("PA:1"),
            Command::SetPlanningAuthorization(false) => Some("PA:0"),
            Command::GetPlanningAutoForceCharge => Some("PAFC:"),
            Command::SetPlanningAutoForceCharge(true) => Some("PAFC:1"),
            Command::SetPlanningAutoForceCharge(false) => Some("PAFC:0"),
            Command::GetDerogation => Some("D:"),
            Command::SetDerogation(_) => Some("D:"),
            Command::GetDeferredStartDelay => Some("Dec:"),
            Command::SetDeferredStartDelay(_) => Some("Dec:"),
            Command::GetSleep => Some("Slp:"),
            Command::SetSleep(_) => Some("Slp:"),
            Command::GetFunctioningMode => Some("FM:"),
            Command::SetFunctioningMode(_) => Some("FM:"),
            Command::GetExternalSignal => Some("FM2:"),
            Command::SetExternalSignal(_) => Some("FM2:"),
            Command::GetEmergencyStop => Some("CAU:"),
            Command::SetEmergencyStop(true) => Some("CAU:1"),
            Command::SetEmergencyStop(false) => Some("CAU:0"),

            // 6. Modbus RS485 & Energy Metering
            Command::GetEnergyMeterPulses => Some("EM:"),
            Command::GetModbusAddress => Some("ModbusAddr:"),
            Command::SetModbusAddress(_) => Some("ModbusAddr:"),
            Command::GetModbusEnergyMeterAddress => Some("ModbusEMA:"),
            Command::SetModbusEnergyMeterAddress(_) => Some("ModbusEMA:"),
            Command::GetModbusDlmAddress => Some("ModbusDLMA:"),
            Command::SetModbusDlmAddress(_) => Some("ModbusDLMA:"),
            Command::GetModbusParity => Some("ModbusP:"),
            Command::SetModbusParity(_) => Some("ModbusP:"),
            Command::GetModbusBaudRate => Some("ModbusB:"),
            Command::SetModbusBaudRate(_) => Some("ModbusB:"),
            Command::GetModbusStopBits => Some("ModbusSB:"),
            Command::SetModbusStopBits(_) => Some("ModbusSB:"),

            // 7. Access Control & RFID Badges
            Command::GetRfidEnabled => Some("RFIDE:"),
            Command::SetRfidEnabled(true) => Some("RFIDE:1"),
            Command::SetRfidEnabled(false) => Some("RFIDE:0"),
            Command::GetRfidUserManagement => Some("RFIDUM:"),
            Command::GetRfidAuthorization => Some("RFIDA:"),
            Command::SendRfidAuthorization(_) => Some("RFIDA:"),
            Command::SendRfidId(_) => Some("RFIDId:"),
            Command::EnrollRfidLocalPass(_) => Some("RFIDIdLP:"),
            Command::GetOcppParameters => Some("OCPPStatus:"),
            Command::SetPlugAndCharge(_) => Some("OCPPPAC:"),
            Command::SetOcppConnectionTimeout(_) => Some("OCPPCTO:"),

            // 8. Bluetooth Controller (BLE)
            Command::GetBluetooth => Some("BT:"),
            Command::SetBluetooth(true) => Some("BT:1"),
            Command::SetBluetooth(false) => Some("BT:0"),
            Command::GetMacAddress => Some("MacAdr:"),

            // 9. Diagnostic & Test Modes
            Command::SetTicTestMode(true) => Some("TICTM:1"),
            Command::SetTicTestMode(false) => Some("TICTM:0"),

            // 10. Raw Debug
            Command::Raw(_) => None,
        }
    }

    /// Determines whether this command expects an acknowledgment from the power board.
    /// Note: Raw commands are non-blocking debug tools and never expect an explicit acknowledgment.
    pub fn expects_response(&self) -> bool {
        self.expected_rx_prefix().is_some()
    }

    /// Checks if a received line from the serial port acknowledges this command.
    ///
    /// Special cases:
    /// - `SBOK` (`Command::SetStartButton(true)`): normally acknowledged by `SB:`, but when the
    ///   charging station is in `State:A` with no vehicle connected, the ATmega returns `SBF:0`.
    /// - `SOK` / `SNOK` (`Command::SetSleep`): acknowledged by `SOK:ok*`, `SNOK:ok*`, or `Slp:`.
    /// - `OCPPPS?` (`Command::GetOcppParameters`): acknowledged by `OCPPStatus:` or `OCPPPS:`.
    pub fn matches_response(&self, rx_line: &str) -> bool {
        match self {
            Command::Ping => {
                let trimmed = rx_line.trim();
                trimmed.starts_with("pong")
            }
            Command::SetStartButton(true) => {
                rx_line.starts_with("SB:") || rx_line.starts_with("SBF:")
            }
            Command::RaspberryPiModeOk => {
                rx_line.starts_with("DIP1:") || rx_line.starts_with("Side:")
            }
            Command::SetSleep(_) => {
                rx_line.starts_with("Slp:")
                    || rx_line.starts_with("SOK:")
                    || rx_line.starts_with("SNOK:")
            }
            Command::GetOcppParameters => {
                rx_line.starts_with("OCPPStatus:") || rx_line.starts_with("OCPPPS:")
            }
            _ => {
                if let Some(prefix) = self.expected_rx_prefix() {
                    rx_line.starts_with(prefix)
                } else {
                    false
                }
            }
        }
    }

    /// Recommended query sequence at startup (excluding `RaspberryPiModeOK` and TIC).
    /// `GetSide` and `GetReference` are prioritized early so the upper layer can identify
    /// the physical charge point side and hardware capabilities immediately.
    pub fn startup_queries() -> Vec<Command> {
        vec![
            // 1. Identity & Boot Handshake
            Command::GetSide,
            Command::GetReference,
            Command::GetSoftwareVersion,
            Command::GetHardwareVersion,
            Command::GetSerialNumber,
            Command::GetWeekYearProduction,
            Command::GetMacAddress,
            Command::GetKitCom,
            Command::GetInitialized,
            Command::GetBluetooth,
            // 2. State Machine & Sensors
            Command::GetState,
            Command::GetErrors,
            Command::GetCpVoltage,
            Command::GetPpCurrent,
            Command::GetCableCurrentCapacity,
            Command::GetFullChargeState,
            Command::GetDomesticPlugPresent,
            Command::GetSbState,
            // 3. Current Limits
            Command::GetCurrentLimit,
            Command::GetSupervisorCurrentLimit,
            Command::GetEliotCurrentLimit,
            Command::GetConsolidatedCurrentLimit,
            Command::GetTicCurrentLimit,
            Command::GetBleShuntCurrentLimit,
            Command::GetBleStandardCurrentLimit,
            // 4. Sockets & Functioning Modes
            Command::GetT2CEnabled,
            Command::GetDomesticEnabled,
            Command::GetForceType2,
            Command::GetForceDomestic,
            Command::GetFunctioningMode,
            Command::GetExternalSignal,
            Command::GetDerogation,
            Command::GetPlanningAuthorization,
            Command::GetPlanningAutoForceCharge,
            Command::GetDeferredStartDelay,
            Command::GetSleep,
            Command::GetEmergencyStop,
            // 5. Metering, RFID, Modbus & OCPP
            Command::GetEnergyMeterPulses,
            Command::GetModbusEnergyMeterAddress,
            Command::GetModbusDlmAddress,
            Command::GetRfidEnabled,
            Command::GetRfidUserManagement,
            Command::GetOcppParameters,
        ]
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_frame())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_simple_frames() {
        assert_eq!(Command::RaspberryPiModeOk.encode(), "RaspberryPiModeOK\r");
        assert_eq!(Command::GetKitCom.encode(), "KitCom?\r");
        assert_eq!(Command::GetInitialized.encode(), "Initialized?\r");
        assert_eq!(Command::GetState.encode(), "State?\r");
        assert_eq!(Command::GetPpCurrent.encode(), "PP?\r");
        assert_eq!(Command::GetCableCurrentCapacity.encode(), "CCCa?\r");
        assert_eq!(Command::GetInstantaneousCurrent.encode(), "CCI?\r");
        assert_eq!(Command::GetFullChargeState.encode(), "FCS?\r");
        assert_eq!(Command::GetDomesticPlugPresent.encode(), "DOM?\r");
        assert_eq!(Command::GetSupervisorCurrentLimit.encode(), "CCS?\r");
        assert_eq!(Command::GetConsolidatedCurrentLimit.encode(), "CCCo?\r");
        assert_eq!(Command::GetBleStandardCurrentLimit.encode(), "CCBLES?\r");
        assert_eq!(Command::GetBleShuntCurrentLimit.encode(), "CCBLESh?\r");
        assert_eq!(Command::GetDomesticEnabled.encode(), "2PC?\r");
        assert_eq!(Command::GetForceType2.encode(), "T2F?\r");
        assert_eq!(Command::GetForceDomestic.encode(), "2PF?\r");
        assert_eq!(Command::GetPlanningAuthorization.encode(), "PA?\r");
        assert_eq!(Command::GetPlanningAutoForceCharge.encode(), "PAFC?\r");
        assert_eq!(Command::GetDeferredStartDelay.encode(), "Dec?\r");
        assert_eq!(Command::GetSleep.encode(), "Slp?\r");
        assert_eq!(Command::GetEmergencyStop.encode(), "CAU?\r");
        assert_eq!(Command::GetEnergyMeterPulses.encode(), "EM?\r");
        assert_eq!(Command::GetModbusAddress.encode(), "ModbusAddr?\r");
        assert_eq!(Command::GetRfidEnabled.encode(), "RFIDE?\r");
        assert_eq!(Command::GetRfidUserManagement.encode(), "RFIDUM?\r");
        assert_eq!(Command::GetRfidAuthorization.encode(), "RFIDA?\r");
        assert_eq!(Command::Unlock.encode(), "Unlock\r");
    }

    #[test]
    fn encodes_ok_nok_pairs() {
        assert_eq!(Command::SetStartButton(false).as_frame(), "SBNOK");
        assert_eq!(Command::AuthorizeType2(true).as_frame(), "T2COK");
        assert_eq!(Command::AuthorizeType2(false).as_frame(), "T2CNOK");
        assert_eq!(Command::AuthorizeDomestic(true).as_frame(), "2PCOK");
        assert_eq!(Command::ForceType2(false).as_frame(), "T2FNOK");
        assert_eq!(Command::SetPlanningAuthorization(true).as_frame(), "PAOK");
        assert_eq!(Command::SetPlanningAuthorization(false).as_frame(), "PANOK");
        assert_eq!(Command::SetPlanningAutoForceCharge(true).as_frame(), "PAFCOK");
        assert_eq!(Command::SetPlanningAutoForceCharge(false).as_frame(), "PAFCNOK");
        assert_eq!(Command::SetSleep(true).as_frame(), "SOK");
        assert_eq!(Command::SetSleep(false).as_frame(), "SNOK");
        assert_eq!(Command::SetEmergencyStop(true).as_frame(), "CAUOK");
        assert_eq!(Command::SetEmergencyStop(false).as_frame(), "CAUNOK");
        assert_eq!(Command::SetRfidEnabled(true).as_frame(), "RFIDEOK");
        assert_eq!(Command::SetRfidEnabled(false).as_frame(), "RFIDENOK");
        assert_eq!(Command::SetBluetooth(false).as_frame(), "BTNOK");
        assert_eq!(Command::SetPlugAndCharge(true).as_frame(), "OCPPPACOK");
    }

    #[test]
    fn encodes_parameterized_frames() {
        assert_eq!(Command::SetCurrentLimit(7).as_frame(), "CCEl:07");
        assert_eq!(Command::SetCurrentLimit(16).as_frame(), "CCEl:16");
        assert_eq!(Command::SetDirectCurrentLimit(20).as_frame(), "CC:20");
        assert_eq!(Command::SetSchukoCurrentLimit(10).as_frame(), "CCS:10");
        assert_eq!(Command::GetSubLineCurrentLimit(1).as_frame(), "CCSL1?");
        assert_eq!(Command::GetSubLineCurrentLimit(4).as_frame(), "CCSL4?");
        assert_eq!(Command::SetDeferredStartDelay(120).as_frame(), "Dec:120");
        assert_eq!(Command::SetModbusAddress(1).as_frame(), "ModbusAddr:1");
        assert_eq!(Command::SetModbusEnergyMeterAddress(2).as_frame(), "ModbusEMA:2");
        assert_eq!(Command::SetModbusDlmAddress(3).as_frame(), "ModbusDLMA:3");
        assert_eq!(Command::SetModbusParity(0).as_frame(), "ModbusP:0");
        assert_eq!(Command::SetModbusBaudRate(2).as_frame(), "ModbusB:2");
        assert_eq!(Command::SetModbusStopBits(1).as_frame(), "ModbusSB:1");
        assert_eq!(Command::EnrollRfidLocalPass("AABBCC".into()).as_frame(), "RFIDIdLP:AABBCC");
        assert_eq!(Command::SetTicTestMode(true).as_frame(), "TICTM:1");
        assert_eq!(Command::SetExternalSignal(false).as_frame(), "FM2:0");
        assert_eq!(
            Command::SetFunctioningMode(FunctioningMode::DirectCharge).as_frame(),
            "FM:1"
        );
        assert_eq!(Command::SendRfidId("04A1B2".into()).as_frame(), "RFIDId:04A1B2");
        assert_eq!(Command::SetOcppConnectionTimeout(120).as_frame(), "OCPPCTO:120");
    }

    #[test]
    fn validates_current_and_subline_ranges() {
        assert!(Command::set_current_limit(6).is_err());
        assert!(Command::set_current_limit(33).is_err());
        assert_eq!(
            Command::set_current_limit(16),
            Ok(Command::SetCurrentLimit(16))
        );
        assert_eq!(
            Command::set_direct_current_limit(16),
            Ok(Command::SetDirectCurrentLimit(16))
        );
        assert!(Command::get_sub_line_current_limit(0).is_err());
        assert!(Command::get_sub_line_current_limit(5).is_err());
        assert_eq!(
            Command::get_sub_line_current_limit(2),
            Ok(Command::GetSubLineCurrentLimit(2))
        );
    }

    #[test]
    fn functioning_mode_roundtrip() {
        for code in 1..=6u8 {
            let m = FunctioningMode::from_code(&code.to_string()).unwrap();
            assert_eq!(m.code(), code);
        }
        assert!(FunctioningMode::from_code("7").is_none());
    }

    #[test]
    fn sbok_matches_sb_and_sbf_in_state_a() {
        let sbok = Command::SetStartButton(true);
        assert!(sbok.matches_response("SB:0"));
        assert!(sbok.matches_response("SB:1"));
        assert!(sbok.matches_response("SBF:0"));
        assert!(sbok.matches_response("SBF:1"));
        assert!(!sbok.matches_response("Default:SBOK"));

        let sbnok = Command::SetStartButton(false);
        assert!(sbnok.matches_response("SB:0"));
        assert!(sbnok.matches_response("SB:1"));
        assert!(!sbnok.matches_response("SBF:0"));
    }

    #[test]
    fn sleep_and_ocpp_status_match_documented_responses() {
        let sok = Command::SetSleep(true);
        assert!(sok.matches_response("SOK:ok*"));
        assert!(sok.matches_response("Slp:1"));
        assert!(!sok.matches_response("State:A"));

        let ocpp = Command::GetOcppParameters;
        assert!(ocpp.matches_response("OCPPStatus:Available"));
        assert!(ocpp.matches_response("OCPPPS:1"));
    }

    #[test]
    fn ping_matches_pong_strictly() {
        let cmd = Command::Ping;
        assert!(cmd.matches_response("pong"));
        assert!(cmd.matches_response("pong\r"));
        assert!(!cmd.matches_response("ping"));
        assert!(!cmd.matches_response("State:A"));
    }

    #[test]
    fn raw_commands_are_non_blocking_for_debug() {
        let raw_query = Command::Raw("CC?".into());
        assert!(!raw_query.expects_response());
        assert!(!raw_query.matches_response("CC:23"));

        let raw_ping = Command::Raw("ping".into());
        assert!(!raw_ping.expects_response());

        let raw_debug = Command::Raw("CUSTOM_DEBUG_FRAME".into());
        assert!(!raw_debug.expects_response());
    }

    #[test]
    fn raspberry_pi_mode_ok_matches_dip1_and_side_strictly_not_rpimode() {
        let cmd = Command::RaspberryPiModeOk;
        assert!(cmd.expects_response());
        assert!(cmd.matches_response("DIP1:1"));
        assert!(cmd.matches_response("DIP1:0"));
        assert!(cmd.matches_response("Side:1"));
        assert!(cmd.matches_response("Side:49"));
        assert!(!cmd.matches_response("RPIMODE"));
        assert!(!cmd.matches_response("RPIMODE:OK"));
        assert!(!cmd.matches_response("State:A"));
        assert!(!cmd.matches_response("CC:32"));
    }

    #[test]
    fn unlock_expects_and_matches_unlock() {
        let cmd = Command::Unlock;
        assert!(cmd.expects_response());
        assert_eq!(cmd.expected_rx_prefix(), Some("Unlock"));
        assert!(cmd.matches_response("Unlock"));
        assert!(cmd.matches_response("Unlock:ok*"));
        assert!(cmd.matches_response("Unlock:nok*"));
        assert!(!cmd.matches_response("State:A"));
    }

    #[test]
    fn all_operational_commands_except_raw_expect_response() {
        for cmd in Command::startup_queries() {
            assert!(
                cmd.expects_response(),
                "Startup query {:?} must expect a response",
                cmd
            );
            assert!(
                cmd.expected_rx_prefix().is_some(),
                "Startup query {:?} must define an expected_rx_prefix",
                cmd
            );
        }
        assert!(!Command::Raw("anything".into()).expects_response());
    }
}
