pub mod commands;
pub mod models;
pub mod parser;

pub use commands::{
    Command, CommandError, FunctioningMode, FRAME_TERMINATOR, MAX_CURRENT_AMPS, MIN_CURRENT_AMPS,
};
pub use models::{BoardInfo, Telemetry};
pub use parser::{parse_line, ProtocolEvent};
