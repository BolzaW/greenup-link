//! Commands issued to the power board (TX).
//!
//! Each variant of [`Command`] corresponds to an ASCII frame documented in
//! `docs/LEGRAND_SERIAL_PROTOCOL.md`. The [`Command::encode`] method produces the
//! frame ready to be written to the serial link (terminated by `\r`).
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
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::CurrentOutOfRange(a) => write!(
                f,
                "current {}A out of bounds ({}..={}A)",
                a, MIN_CURRENT_AMPS, MAX_CURRENT_AMPS
            ),
        }
    }
}

impl std::error::Error for CommandError {}

/// All known commands that can be sent to the power board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    // --- 1. Initialization & System ---
    /// `RaspberryPiModeOK` – Announces that the Pi is taking control (response to the `RaspberryPi?` ping).
    RaspberryPiModeOk,
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
    /// `Side?` – Active side (dual-socket stations).
    GetSide,
    /// `Reset` – Reboots the power board.
    Reset,
    /// `Test` – Factory test mode.
    FactoryTest,
    /// `ping` – Basic ping.
    Ping,

    // --- 2. Status & Telemetry ---
    /// `State?`
    GetState,
    /// `E?`
    GetErrors,
    /// `CC?`
    GetCurrentLimit,
    /// `CCEl?`
    GetEliotCurrentLimit,
    /// `CP?`
    GetCpVoltage,
    /// `T2C?`
    GetT2CEnabled,
    /// `SB?`
    GetSbState,
    /// `CCTIC?` – Current limit calculated from the TIC.
    GetTicCurrentLimit,
    /// `TICTM:1` / `TICTM:0` – Enables/disables TIC test mode.
    SetTicTestMode(bool),

    // --- 3. Charge Control ---
    /// `CC:XX` – Type 2 socket current limit (use [`Command::set_current_limit`] to validate).
    SetCurrentLimit(u8),
    /// `CCS:XX` – Schuko socket current limit.
    SetSchukoCurrentLimit(u8),
    /// `T2COK` / `T2CNOK` – Authorizes/blocks Type 2 charging.
    AuthorizeType2(bool),
    /// `2PCOK` / `2PCNOK` – Authorizes/blocks domestic socket charging.
    AuthorizeDomestic(bool),
    /// `T2FOK` / `T2FNOK` – Forces Type 2 charging.
    ForceType2(bool),
    /// `2PFOK` / `2PFNOK` – Forces domestic socket charging.
    ForceDomestic(bool),
    /// `SOK` / `SNOK` – Enters/exits sleep mode (only from state A).
    SetSleep(bool),
    /// `SBOK` / `SBNOK` - Simulates a software press on the START/STOP button (Perfect Pause/Resume).
    SetStartButton(bool),
    /// `Unlock` – Physically unlocks the cable.
    Unlock,

    // --- 4. OCPP / RFID ---
    /// `OCPPPS?`
    GetOcppParameters,
    /// `OCPPPACOK` / `OCPPPACNOK` – Plug & Charge.
    SetPlugAndCharge(bool),
    /// `RFIDId:XXXX` – Transmits a badge identifier.
    SendRfidId(String),
    /// `RFIDA:XXXX` – Transmits an RFID authorization status.
    SendRfidAuthorization(String),
    /// `OCPPCTO:XXX` – OCPP connection timeout.
    SetOcppConnectionTimeout(u32),

    // --- 5. Bluetooth ---
    /// `BT?`
    GetBluetooth,
    /// `BTOK` / `BTNOK`
    SetBluetooth(bool),

    // --- 7. Functioning Modes ---
    /// `FM?`
    GetFunctioningMode,
    /// `FM:X`
    SetFunctioningMode(FunctioningMode),
    /// `FM2?` – Eco-Start state.
    GetExternalSignal,
    /// `FM2:1` / `FM2:0` – Enables/disables Eco-Start (DIP2 equivalent).
    SetExternalSignal(bool),
    /// `D?` – Derogation state (front button force charge).
    GetDerogation,
    /// `D:1` / `D:0` – Enables/disables derogation (front button force charge).
    SetDerogation(bool),

    // --- 8. Debug ---
    /// Sending a raw command without expecting an acknowledgment (debug API).
    Raw(String),
}

fn ok_nok(prefix: &str, enabled: bool) -> String {
    format!("{}{}", prefix, if enabled { "OK" } else { "NOK" })
}

impl Command {
    /// Builds a `CC:XX` command while validating the current range.
    pub fn set_current_limit(amps: u8) -> Result<Self, CommandError> {
        Self::validate_current(amps).map(Command::SetCurrentLimit)
    }

    /// Builds a `CCS:XX` command while validating the current range.
    pub fn set_schuko_current_limit(amps: u8) -> Result<Self, CommandError> {
        Self::validate_current(amps).map(Command::SetSchukoCurrentLimit)
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
            Command::RaspberryPiModeOk => "RaspberryPiModeOK".into(),
            Command::GetSoftwareVersion => "SoftwareVersion?".into(),
            Command::GetHardwareVersion => "HardwareVersion?".into(),
            Command::GetSerialNumber => "SerialNumber?".into(),
            Command::GetReference => "Reference?".into(),
            Command::GetWeekYearProduction => "WeekYearProduction?".into(),
            Command::GetSide => "Side?".into(),
            Command::Reset => "Reset".into(),
            Command::FactoryTest => "Test".into(),
            Command::Ping => "ping".into(),

            Command::GetState => "State?".into(),
            Command::GetErrors => "E?".into(),
            Command::GetCurrentLimit => "CC?".into(),
            Command::GetEliotCurrentLimit => "CCEl?".into(),
            Command::GetCpVoltage => "CP?".into(),
            Command::GetT2CEnabled => "T2C?".into(),
            Command::GetSbState => "SB?".into(),
            Command::GetTicCurrentLimit => "CCTIC?".into(),
            Command::SetTicTestMode(on) => format!("TICTM:{}", *on as u8),

            Command::SetCurrentLimit(a) => format!("CCEl:{:02}", a),
            Command::SetSchukoCurrentLimit(a) => format!("CCS:{:02}", a),
            Command::AuthorizeType2(on) => ok_nok("T2C", *on),
            Command::AuthorizeDomestic(on) => ok_nok("2PC", *on),
            Command::ForceType2(on) => ok_nok("T2F", *on),
            Command::ForceDomestic(on) => ok_nok("2PF", *on),
            Command::SetSleep(on) => ok_nok("S", *on),
            Command::SetStartButton(on) => ok_nok("SB", *on),
            Command::Unlock => "Unlock".into(),

            Command::GetOcppParameters => "OCPPPS?".into(),
            Command::SetPlugAndCharge(on) => ok_nok("OCPPPAC", *on),
            Command::SendRfidId(id) => format!("RFIDId:{}", id),
            Command::SendRfidAuthorization(s) => format!("RFIDA:{}", s),
            Command::SetOcppConnectionTimeout(t) => format!("OCPPCTO:{}", t),

            Command::GetBluetooth => "BT?".into(),
            Command::SetBluetooth(on) => ok_nok("BT", *on),

            Command::GetFunctioningMode => "FM?".into(),
            Command::SetFunctioningMode(m) => format!("FM:{}", m.code()),
            Command::GetExternalSignal => "FM2?".into(),
            Command::SetExternalSignal(on) => format!("FM2:{}", *on as u8),
            Command::GetDerogation => "D?".into(),
            Command::SetDerogation(accept) => if *accept { "DOK".into() } else { "DNOK".into() },
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
            Command::RaspberryPiModeOk => Some("Side:"),
            Command::GetSoftwareVersion => Some("SoftwareVersion:"),
            Command::GetHardwareVersion => Some("HardwareVersion:"),
            Command::GetSerialNumber => Some("SerialNumber:"),
            Command::GetReference => Some("Reference:"),
            Command::GetWeekYearProduction => Some("WeekYearProduction:"),
            Command::GetSide => Some("Side:"),
            Command::Reset => Some("State:"), // Reset returns the state after reboot
            Command::FactoryTest => None,
            Command::Ping => Some("pong"),

            Command::GetState => Some("State:"),
            Command::GetErrors => Some("E:"),
            Command::GetCurrentLimit => Some("CC:"),
            Command::GetEliotCurrentLimit => Some("CCEl:"),
            Command::GetCpVoltage => Some("CP:"),
            Command::GetT2CEnabled => Some("T2C:"),
            Command::GetSbState => Some("SB:"),
            Command::GetTicCurrentLimit => Some("CCTIC:"),
            Command::SetTicTestMode(true) => Some("TICTM:1"),
            Command::SetTicTestMode(false) => Some("TICTM:0"),

            Command::SetCurrentLimit(_) => Some("CCEl:"),
            Command::SetSchukoCurrentLimit(_) => Some("CCS:"),
            Command::AuthorizeType2(true) => Some("T2C:1"),
            Command::AuthorizeType2(false) => Some("T2C:0"),
            Command::AuthorizeDomestic(true) => Some("2PC:1"),
            Command::AuthorizeDomestic(false) => Some("2PC:0"),
            Command::ForceType2(true) => Some("T2F:1"),
            Command::ForceType2(false) => Some("T2F:0"),
            Command::ForceDomestic(true) => Some("2PF:1"),
            Command::ForceDomestic(false) => Some("2PF:0"),
            Command::SetSleep(_) => Some("Slp:"),
            Command::SetStartButton(_) => Some("SB:"),
            Command::Unlock => None, // To verify

            Command::GetOcppParameters => Some("OCPPPS:"),
            Command::SetPlugAndCharge(_) => Some("OCPPPAC:"),
            Command::SendRfidId(_) => Some("RFIDId:"),
            Command::SendRfidAuthorization(_) => Some("RFIDA:"),
            Command::SetOcppConnectionTimeout(_) => Some("OCPPCTO:"),

            Command::GetBluetooth => Some("BT:"),
            Command::SetBluetooth(true) => Some("BT:1"),
            Command::SetBluetooth(false) => Some("BT:0"),

            Command::GetFunctioningMode => Some("FM:"),
            Command::SetFunctioningMode(_) => Some("FM:"),
            Command::GetExternalSignal => Some("FM2:"),
            Command::SetExternalSignal(_) => Some("FM2:"),
            Command::GetDerogation => Some("D:"),
            Command::SetDerogation(_) => Some("D:"),
            Command::Raw(_) => None,
        }
    }

    /// Recommended query sequence at startup (excluding `RaspberryPiModeOK` and TIC).
    pub fn startup_queries() -> Vec<Command> {
        vec![
            Command::GetSoftwareVersion,
            Command::GetHardwareVersion,
            Command::GetSerialNumber,
            Command::GetReference,
            Command::GetWeekYearProduction,
            Command::GetState,
            Command::GetFunctioningMode,
            Command::GetExternalSignal,
            Command::GetDerogation,
            Command::GetCurrentLimit,
            Command::GetEliotCurrentLimit,
            Command::GetCpVoltage,
            Command::GetT2CEnabled,
            Command::GetSbState,
            Command::GetErrors,
            Command::GetBluetooth,
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
        assert_eq!(Command::GetState.encode(), "State?\r");
        assert_eq!(Command::Unlock.encode(), "Unlock\r");
    }

    #[test]
    fn encodes_ok_nok_pairs() {
        assert_eq!(Command::SetStartButton(false).as_frame(), "SBNOK");
        assert_eq!(Command::AuthorizeType2(true).as_frame(), "T2COK");
        assert_eq!(Command::AuthorizeType2(false).as_frame(), "T2CNOK");
        assert_eq!(Command::AuthorizeDomestic(true).as_frame(), "2PCOK");
        assert_eq!(Command::ForceType2(false).as_frame(), "T2FNOK");
        assert_eq!(Command::SetSleep(true).as_frame(), "SOK");
        assert_eq!(Command::SetBluetooth(false).as_frame(), "BTNOK");
        assert_eq!(Command::SetPlugAndCharge(true).as_frame(), "OCPPPACOK");
    }

    #[test]
    fn encodes_parameterized_frames() {
        assert_eq!(Command::SetCurrentLimit(7).as_frame(), "CCEl:07");
        assert_eq!(Command::SetCurrentLimit(16).as_frame(), "CCEl:16");
        assert_eq!(Command::SetSchukoCurrentLimit(10).as_frame(), "CCS:10");
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
    fn validates_current_range() {
        assert!(Command::set_current_limit(9).is_err());
        assert!(Command::set_current_limit(33).is_err());
        assert_eq!(
            Command::set_current_limit(16),
            Ok(Command::SetCurrentLimit(16))
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
}
