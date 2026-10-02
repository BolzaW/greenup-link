pub mod models;
pub mod parser;

pub use models::{BoardInfo, Telemetry};
pub use parser::{parse_line, ProtocolEvent};
