pub mod logger;
pub mod raw_state;
pub mod serial;
pub mod state;

pub use raw_state::{AtmegaRawState, DriverSnapshot, SerialStatus, UsbLinkState};
pub use serial::{
    check_tic_state_a, discover_candidate_ports, parse_incoming_line, run_serial_loop,
    run_serial_loop_with_options, spawn_port, start_tic_detection, trigger_init_sequence,
    SerialOptions, TicStartError,
};
pub use state::{DriverHandle, SharedDriver, TicDetectionState};
