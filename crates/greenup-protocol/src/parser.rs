#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolEvent {
    Ping,
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
    TicTestBaud(String),
    TicTestInit,
    Voltage(f32),
    Current(f32),
    LimitAmps(u32),
    Energy(f32),
    Frequency(f32),
    Unknown(String),
}

pub fn parse_line(line: &str) -> ProtocolEvent {
    if line.starts_with("RaspberryPi?") {
        ProtocolEvent::Ping
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
    } else if line.starts_with("CCI:") {
        if let Ok(val) = line.replace("CCI:", "").parse::<f32>() {
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
    } else if line.starts_with("Ener:") {
        if let Ok(val) = line.replace("Ener:", "").parse::<f32>() {
            ProtocolEvent::Energy(val)
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else if line.starts_with("Freq:") {
        if let Ok(val) = line.replace("Freq:", "").parse::<f32>() {
            ProtocolEvent::Frequency(val)
        } else {
            ProtocolEvent::Unknown(line.to_string())
        }
    } else {
        ProtocolEvent::Unknown(line.to_string())
    }
}
