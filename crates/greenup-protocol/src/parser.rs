#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolEvent {
    Ping,
    Pong,
    SoftwareVersion(String),
    HardwareVersion(String),
    SerialNumber(String),
    Reference(String),
    WeekYearProduction(String),
    BluetoothState(bool),
    StateChange(String),
    ErrorChange(String),
    ChargeComplete,
    FmMode(String),
    ExternalSignal(bool),
    Derogation(bool),
    TicTestBaud(String),
    TicTestInit,
    Voltage(f32),
    Current(f32),
    LimitAmps(u32),
    EliotLimitAmps(u32),
    CpVoltage(u32),
    T2CEnabled(bool),
    SbState(bool),
    SbfState(bool),
    Energy(f32),
    Frequency(f32),
    MacAddress(String),
    Side(String),
    Dip1(String),
    ChargePhases(String),
    SessionTicket(String),
    CommandNotUnderstood(String),
    Unknown(String),
}

pub fn parse_line(line: &str) -> ProtocolEvent {
    let trimmed = line.trim();
    if trimmed.starts_with("RaspberryPi?") {
        ProtocolEvent::Ping
    } else if trimmed == "pong" {
        ProtocolEvent::Pong
    } else if line.starts_with("SoftwareVersion:") {
        ProtocolEvent::SoftwareVersion(line.replace("SoftwareVersion:", ""))
    } else if line.starts_with("HardwareVersion:") {
        ProtocolEvent::HardwareVersion(line.replace("HardwareVersion:", ""))
    } else if line.starts_with("SerialNumber:") {
        ProtocolEvent::SerialNumber(line.replace("SerialNumber:", ""))
    } else if line.starts_with("Reference:") {
        ProtocolEvent::Reference(line.replace("Reference:", ""))
    } else if line.starts_with("WeekYearProduction:") {
        ProtocolEvent::WeekYearProduction(line.replace("WeekYearProduction:", ""))
    } else if line.starts_with("BT:") {
        let val = line.replace("BT:", "");
        ProtocolEvent::BluetoothState(val == "1")
    } else if line.starts_with("State:") {
        ProtocolEvent::StateChange(line.replace("State:", ""))
    } else if line.starts_with("E:") {
        ProtocolEvent::ErrorChange(line.replace("E:", ""))
    } else if line.starts_with("FCS:") {
        ProtocolEvent::ChargeComplete
    } else if line.starts_with("FM:") {
        ProtocolEvent::FmMode(line.replace("FM:", ""))
    } else if line.starts_with("FM2:") {
        ProtocolEvent::ExternalSignal(line.replace("FM2:", "") == "1")
    } else if line.starts_with("D:") {
        ProtocolEvent::Derogation(line.replace("D:", "") == "1")
    } else if line.starts_with("TICTestB:") {
        ProtocolEvent::TicTestBaud(line.replace("TICTestB:", ""))
    } else if line.starts_with("TICTestC:") {
        ProtocolEvent::TicTestInit
    } else if line.starts_with("Volt:") {
        if let Ok(val) = line.replace("Volt:", "").parse::<f32>() {
            ProtocolEvent::Voltage(val)
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if let Some(val_str) = line.strip_prefix("CCI:") {
        let first_val = val_str.split(';').next().unwrap_or(val_str).trim();
        if let Ok(val) = first_val.parse::<f32>() {
            ProtocolEvent::Current(val)
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if line.starts_with("CC:") {
        if let Ok(val) = line.replace("CC:", "").parse::<u32>() {
            ProtocolEvent::LimitAmps(val)
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if line.starts_with("CCEl:") {
        if let Ok(val) = line.replace("CCEl:", "").parse::<u32>() {
            ProtocolEvent::EliotLimitAmps(val)
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
        } else if line.starts_with("CP:") {
        if let Ok(val) = line.replace("CP:", "").parse::<u32>() {
            ProtocolEvent::CpVoltage(val)
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if line.starts_with("T2C:") {
        let v = line.replace("T2C:", "");
        if v == "1" || v == "0" {
            ProtocolEvent::T2CEnabled(v == "1")
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if line.starts_with("SB:") {
        let v = line.replace("SB:", "");
        if v == "1" || v == "0" {
            ProtocolEvent::SbState(v == "1")
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if line.starts_with("SBF:") {
        let v = line.replace("SBF:", "");
        if v == "1" || v == "0" {
            ProtocolEvent::SbfState(v == "1")
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if line.starts_with("Ener:") {
        if let Ok(val) = line.replace("Ener:", "").parse::<f32>() {
            ProtocolEvent::Energy(val)
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if let Some(val) = line.strip_prefix("MacAdr:") {
        ProtocolEvent::MacAddress(val.trim().to_string())
    } else if let Some(val) = line.strip_prefix("Side:") {
        ProtocolEvent::Side(val.trim().to_string())
    } else if let Some(val) = line.strip_prefix("DIP1:") {
        ProtocolEvent::Dip1(val.trim().to_string())
    } else if line.starts_with("Freq:") {
        if let Ok(val) = line.replace("Freq:", "").parse::<f32>() {
            ProtocolEvent::Frequency(val)
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if let Some(val) = line.strip_prefix("CPh:").or_else(|| line.strip_prefix("Cph:")) {
        ProtocolEvent::ChargePhases(val.trim().to_string())
    } else if line.starts_with("WT:") {
        ProtocolEvent::SessionTicket(line.trim().to_string())
    } else if let Some(cmd) = line.strip_prefix("Default:") {
        ProtocolEvent::CommandNotUnderstood(cmd.trim().to_string())
    } else {
        ProtocolEvent::Unknown(line.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mac_address_frames() {
        assert_eq!(
            parse_line("MacAdr:000000000000"),
            ProtocolEvent::MacAddress("000000000000".into())
        );
        assert_eq!(
            parse_line("MacAdr:001A7DDA7113"),
            ProtocolEvent::MacAddress("001A7DDA7113".into())
        );
        assert_eq!(
            parse_line("Side:49"),
            ProtocolEvent::Side("49".into())
        );
        assert_eq!(
            parse_line("DIP1:1"),
            ProtocolEvent::Dip1("1".into())
        );
        assert_eq!(
            parse_line("CCI:24.48"),
            ProtocolEvent::Current(24.48)
        );
        assert_eq!(
            parse_line("CCI:24.42;0.00;0.00"),
            ProtocolEvent::Current(24.42)
        );
        assert_eq!(
            parse_line("CPh:Mono"),
            ProtocolEvent::ChargePhases("Mono".into())
        );
        assert_eq!(
            parse_line("Cph:Tri"),
            ProtocolEvent::ChargePhases("Tri".into())
        );
        assert_eq!(parse_line("pong"), ProtocolEvent::Pong);
        assert_eq!(parse_line("pong\r"), ProtocolEvent::Pong);
        assert_eq!(parse_line("ping"), ProtocolEvent::Unknown("ping".into()));
    }
}
