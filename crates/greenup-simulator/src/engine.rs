use rand::Rng;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub side: u8,
    pub reference: String,
    pub serial_number: String,
    pub software_version: String,
    pub hardware_version: String,
    pub week_year_production: String,
    pub phases: u8,
    pub max_hardware_amps: u8,
    pub has_schuko: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            side: 1,
            reference: "058001".to_string(),
            serial_number: "SIM2026102901".to_string(),
            software_version: "V01;17;77".to_string(),
            hardware_version: "V01;01".to_string(),
            week_year_production: "4226".to_string(),
            phases: 1,
            max_hardware_amps: 32,
            has_schuko: false,
        }
    }
}

impl EngineConfig {
    /// Preset configuration based on official Legrand commercial reference.
    pub fn for_reference(reference: &str, side: u8) -> Self {
        let clean = reference.replace(' ', "");
        let mut cfg = Self::default();
        cfg.side = side;
        cfg.reference = clean.clone();
        cfg.serial_number = format!("SIM{}{:02}", clean, side);

        match clean.as_str() {
            // Dual point references (12 official Legrand metal models)
            "058011" | "059011" | "058042" | "059042" => {
                cfg.phases = 1;
                cfg.max_hardware_amps = 20;
                cfg.has_schuko = true;
            }
            "058013" | "059013" | "058044" | "059044" => {
                cfg.phases = 1;
                cfg.max_hardware_amps = 32;
                cfg.has_schuko = true;
            }
            "058015" | "059015" | "058049" | "059049" => {
                cfg.phases = 3;
                cfg.max_hardware_amps = 32;
                cfg.has_schuko = true;
            }
            // Single point 3.7 - 4.6 kW models
            "058000" | "059000" | "059005" | "058003" | "059003" | "059008" => {
                cfg.phases = 1;
                cfg.max_hardware_amps = 20;
            }
            // Three-phase single point
            "058002" | "059002" | "059007" | "058014" | "059014" => {
                cfg.phases = 3;
                cfg.max_hardware_amps = 32;
            }
            // Single point 7.4 kW default (058001, 059001, 059006, etc.)
            _ => {
                cfg.phases = 1;
                cfg.max_hardware_amps = 32;
            }
        }
        cfg
    }
}

#[derive(Debug, Clone)]
pub struct EngineState {
    pub state: char,
    pub cp_volts: f32,
    pub pp_amps: u8,
    pub contactor_closed: bool,
    pub cc_limit: u8,
    pub car_max_amps: u8,
    pub measured_current: f32,
    pub voltage: f32,
    pub energy_kwh: f32,
    pub error_code: String,
    pub t2c_authorized: bool,
    pub start_button: bool,
    pub functioning_mode: u8,
    pub external_signal_fm2: u8,
    pub derogation: bool,
    pub tic_test_mode: bool,
    pub rpi_mode: bool,
    pub last_tick: Instant,
}

impl Default for EngineState {
    fn default() -> Self {
        Self {
            state: 'A',
            cp_volts: 12.0,
            pp_amps: 0,
            contactor_closed: false,
            cc_limit: 32,
            car_max_amps: 32,
            measured_current: 0.0,
            voltage: 230.0,
            energy_kwh: 0.0,
            error_code: "0000".to_string(),
            t2c_authorized: true,
            start_button: true,
            functioning_mode: 1,
            external_signal_fm2: 0,
            derogation: false,
            tic_test_mode: false,
            rpi_mode: false,
            last_tick: Instant::now(),
        }
    }
}

pub struct AtmegaEngine {
    pub config: EngineConfig,
    pub state: EngineState,
}

impl AtmegaEngine {
    pub fn new(config: EngineConfig) -> Self {
        let max_amps = config.max_hardware_amps;
        let mut state = EngineState::default();
        state.cc_limit = max_amps;
        state.car_max_amps = max_amps;
        Self { config, state }
    }

    /// Target charging current computed from all constraints.
    pub fn effective_target_amps(&self) -> f32 {
        if self.state.state != 'C' || !self.state.contactor_closed {
            return 0.0;
        }
        let limit = self
            .state
            .cc_limit
            .min(self.state.car_max_amps)
            .min(self.state.pp_amps)
            .min(self.config.max_hardware_amps);
        limit as f32
    }

    /// Simulates physical electrical dynamics (current ramp, noise, energy accumulation).
    /// Typically called every 100ms or 1s.
    pub fn tick(&mut self) -> Option<String> {
        let now = Instant::now();
        let dt_secs = now.duration_since(self.state.last_tick).as_secs_f32().max(0.001);
        self.state.last_tick = now;
        self.tick_with_dt(dt_secs)
    }

    /// Simulates physics tick with explicit delta time in seconds.
    pub fn tick_with_dt(&mut self, dt_secs: f32) -> Option<String> {
        let mut rng = rand::thread_rng();

        if self.state.state == 'C' && self.state.contactor_closed {
            let target = self.effective_target_amps();

            // Gradual ramp-up (approx 3 A/s) or ramp-down (approx 5 A/s)
            if self.state.measured_current < target {
                let step = 3.0 * dt_secs;
                if self.state.measured_current == 0.0 {
                    self.state.measured_current = 0.15; // Initial connection current
                } else {
                    self.state.measured_current = (self.state.measured_current + step).min(target);
                }
            } else if self.state.measured_current > target {
                let step = 5.0 * dt_secs;
                self.state.measured_current = (self.state.measured_current - step).max(target);
            }

            // Realistic sensor noise: +/- 0.15 A jitter when near target
            let noise: f32 = if self.state.measured_current > 1.0 {
                rng.gen_range(-0.15..0.15)
            } else {
                0.0
            };
            let live_current = (self.state.measured_current + noise).max(0.0);

            // Realistic voltage micro-fluctuation around 230V (+/- 1.5V)
            self.state.voltage = 230.0 + rng.gen_range(-1.5..1.5);

            // Energy integration: P = U * I (or 3 * U * I in 3-phase)
            let power_w = if self.config.phases == 3 {
                3.0 * self.state.voltage * live_current
            } else {
                self.state.voltage * live_current
            };
            self.state.energy_kwh += (power_w * dt_secs) / (3600.0 * 1000.0);

            // Returns spontaneous CCI frame
            if self.config.phases == 3 {
                Some(format!("CCI:{:.2};{:.2};{:.2}", live_current, live_current, live_current))
            } else {
                Some(format!("CCI:{:.2}", live_current))
            }
        } else {
            // Contactor open or not in State C
            self.state.measured_current = 0.0;
            None
        }
    }

    /// Event: Vehicle cable plugged into the charging station.
    pub fn event_plug(&mut self) -> Vec<String> {
        if self.state.state != 'A' && self.state.state != 'L' {
            return vec![];
        }
        self.state.state = 'B';
        self.state.cp_volts = 9.0;
        self.state.pp_amps = self.config.max_hardware_amps; // 32A or 20A cable detected

        vec![
            format!("CCCa:{}", self.state.pp_amps),
            "CCS:32".to_string(),
            "SBF:1".to_string(),
            "Start".to_string(),
            "State:B".to_string(),
        ]
    }

    /// Event: Vehicle ready and authorizes charging (closes contactor).
    pub fn event_charge(&mut self) -> Vec<String> {
        if self.state.state != 'B' {
            return vec![];
        }
        self.state.state = 'C';
        self.state.cp_volts = 6.0;
        self.state.contactor_closed = true;

        vec![
            "State:C".to_string(),
            format!("CC:{:02}", self.state.cc_limit),
        ]
    }

    /// Event: Vehicle interrupts / suspends charging (e.g. battery 100% or user stopped in-car).
    pub fn event_pause_car(&mut self) -> Vec<String> {
        if self.state.state != 'C' {
            return vec![];
        }
        self.state.state = 'B';
        self.state.cp_volts = 9.0;
        self.state.contactor_closed = false;
        self.state.measured_current = 0.0;

        vec!["State:B".to_string()]
    }

    /// Event: Vehicle resumes charging after pause.
    pub fn event_resume_car(&mut self) -> Vec<String> {
        if self.state.state != 'B' || self.state.pp_amps == 0 {
            return vec![];
        }
        self.state.state = 'C';
        self.state.cp_volts = 6.0;
        self.state.contactor_closed = true;

        vec![
            "State:C".to_string(),
            format!("CC:{:02}", self.state.cc_limit),
        ]
    }

    /// Event: Cable unplugged from vehicle/station.
    pub fn event_unplug(&mut self) -> Vec<String> {
        if self.state.state == 'A' {
            return vec![];
        }
        self.state.contactor_closed = false;
        self.state.measured_current = 0.0;
        self.state.cp_volts = 12.0;
        self.state.pp_amps = 0;
        self.state.state = 'A';

        vec![
            "State:L".to_string(),
            "SBF:0".to_string(),
            "State:A".to_string(),
        ]
    }

    /// Event: Hardware fault injection (e.g. E:0010 Overcurrent).
    pub fn event_fault(&mut self, error_code: &str) -> Vec<String> {
        self.state.contactor_closed = false;
        self.state.measured_current = 0.0;
        self.state.state = 'E';
        self.state.cp_volts = 0.0;
        self.state.error_code = error_code.to_string();

        vec![
            format!("E:{}", self.state.error_code),
            "State:E".to_string(),
        ]
    }

    /// Event: Clear fault.
    pub fn event_clear_fault(&mut self) -> Vec<String> {
        self.state.error_code = "0000".to_string();
        if self.state.pp_amps > 0 {
            self.state.state = 'B';
            self.state.cp_volts = 9.0;
            vec!["E:0000".to_string(), "State:B".to_string()]
        } else {
            self.state.state = 'A';
            self.state.cp_volts = 12.0;
            vec!["E:0000".to_string(), "State:A".to_string()]
        }
    }

    /// Event: Set vehicle onboard charger max current.
    pub fn set_car_limit(&mut self, amps: u8) {
        self.state.car_max_amps = amps;
    }

    /// Processes an ASCII command frame received on the serial port (master PTY).
    /// Returns response frames to send back to the host.
    pub fn handle_command(&mut self, raw_cmd: &str) -> Vec<String> {
        let cmd = raw_cmd.trim();

        // 1. Handshake & Identifiers
        if cmd == "RaspberryPiModeOK" {
            self.state.rpi_mode = true;
            return vec!["DIP1:1".to_string()];
        }
        if cmd == "Side?" {
            let side_val = if self.config.side == 1 { 49 } else { 50 };
            return vec![format!("Side:{}", side_val)];
        }
        if cmd == "Reference?" {
            return vec![format!("Reference:{}", self.config.reference)];
        }
        if cmd == "SoftwareVersion?" {
            return vec![format!("SoftwareVersion:{}", self.config.software_version)];
        }
        if cmd == "HardwareVersion?" {
            return vec![format!("HardwareVersion:{}", self.config.hardware_version)];
        }
        if cmd == "SerialNumber?" {
            return vec![format!("SerialNumber:{}", self.config.serial_number)];
        }
        if cmd == "WeekYearProduction?" {
            return vec![format!("WeekYearProduction:{}", self.config.week_year_production)];
        }

        // 2. Telemetry & State Queries
        if cmd == "State?" {
            return vec![format!("State:{}", self.state.state)];
        }
        if cmd == "CP?" {
            return vec![format!("CP:{:.0}", self.state.cp_volts)];
        }
        if cmd == "PP?" {
            return vec![format!("PP:{}", self.state.pp_amps)];
        }
        if cmd == "E?" {
            return vec![format!("E:{}", self.state.error_code)];
        }
        if cmd == "CCI?" {
            if self.config.phases == 3 {
                return vec![format!(
                    "CCI:{:.2};{:.2};{:.2}",
                    self.state.measured_current, self.state.measured_current, self.state.measured_current
                )];
            }
            return vec![format!("CCI:{:.2}", self.state.measured_current)];
        }

        // 3. Current Setting & Inquiries
        if cmd == "CC?" {
            return vec![format!("CC:{:02}", self.state.cc_limit)];
        }
        if let Some(val_str) = cmd.strip_prefix("CC:") {
            if let Ok(val) = val_str.parse::<u8>() {
                self.state.cc_limit = val;
                return vec![format!("CC:{:02}", val)];
            }
        }
        if cmd == "CCEl?" {
            return vec![format!("CCEl:{:02}", self.state.cc_limit)];
        }
        if let Some(val_str) = cmd.strip_prefix("CCEl:") {
            if let Ok(val) = val_str.parse::<u8>() {
                self.state.cc_limit = val;
                return vec![format!("CCEl:{:02}", val)];
            }
        }
        if cmd == "CCCa?" {
            return vec![format!("CCCa:{}", self.state.pp_amps)];
        }
        if cmd == "CCS?" {
            return vec!["CCS:16".to_string()];
        }
        if let Some(val_str) = cmd.strip_prefix("CCS:") {
            return vec![format!("CCS:{}", val_str)];
        }
        if cmd == "CCTIC?" {
            return vec!["CCTIC:32".to_string()];
        }

        // 4. Functioning Modes
        if cmd == "FM?" {
            return vec![format!("FM:{}", self.state.functioning_mode)];
        }
        if let Some(val_str) = cmd.strip_prefix("FM:") {
            if let Ok(val) = val_str.parse::<u8>() {
                self.state.functioning_mode = val;
                return vec![format!("FM:{}", val)];
            }
        }
        if cmd == "FM2?" {
            return vec![format!("FM2:{}", self.state.external_signal_fm2)];
        }
        if let Some(val_str) = cmd.strip_prefix("FM2:") {
            if let Ok(val) = val_str.parse::<u8>() {
                self.state.external_signal_fm2 = val;
                return vec![format!("FM2:{}", val)];
            }
        }
        if cmd == "D?" {
            return vec![format!("D:{}", if self.state.derogation { 1 } else { 0 })];
        }
        if cmd == "DOK" {
            self.state.derogation = true;
            return vec!["D:1".to_string()];
        }
        if cmd == "DNOK" {
            self.state.derogation = false;
            return vec!["D:0".to_string()];
        }

        // 5. Hardware Controls & Pause/Resume
        if cmd == "SBNOK" {
            self.state.start_button = false;
            if self.state.state == 'C' {
                self.state.state = 'M';
                self.state.contactor_closed = false;
                self.state.measured_current = 0.0;
                return vec!["State:M".to_string()];
            }
            return vec!["SB:0".to_string()];
        }
        if cmd == "SBOK" {
            self.state.start_button = true;
            if self.state.state == 'M' {
                if self.state.pp_amps > 0 && self.state.t2c_authorized {
                    self.state.state = 'C';
                    self.state.contactor_closed = true;
                    return vec!["State:B".to_string(), "State:C".to_string()];
                }
                self.state.state = 'B';
                return vec!["State:B".to_string()];
            }
            if self.state.state == 'A' {
                return vec!["SBF:0".to_string()];
            }
            return vec!["SB:1".to_string()];
        }
        if cmd == "T2COK" {
            self.state.t2c_authorized = true;
            return vec!["T2C:1".to_string()];
        }
        if cmd == "T2CNOK" {
            self.state.t2c_authorized = false;
            if self.state.state == 'C' {
                self.state.state = 'B';
                self.state.contactor_closed = false;
                self.state.measured_current = 0.0;
                return vec!["T2C:0".to_string(), "State:B".to_string()];
            }
            return vec!["T2C:0".to_string()];
        }
        if cmd == "TICTM:1" {
            self.state.tic_test_mode = true;
            return vec!["TICTM:1".to_string()];
        }
        if cmd == "TICTM:0" {
            self.state.tic_test_mode = false;
            return vec!["TICTM:0".to_string()];
        }
        if cmd == "Reset" {
            self.state = EngineState::default();
            return vec!["State:A".to_string()];
        }
        if cmd == "ping" {
            return vec!["pong".to_string()];
        }
        if cmd == "BluetoothRadioStatus?" {
            return vec!["BluetoothRadioStatus:1".to_string()];
        }

        // Default response for unhandled commands (as done by the real ATmega)
        vec![format!("Default:{}", cmd)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_init_and_handshake() {
        let mut engine = AtmegaEngine::new(EngineConfig::for_reference("058001", 1));
        assert_eq!(engine.handle_command("RaspberryPiModeOK"), vec!["DIP1:1"]);
        assert_eq!(engine.handle_command("Side?"), vec!["Side:49"]);
        assert_eq!(engine.handle_command("Reference?"), vec!["Reference:058001"]);
        assert_eq!(engine.handle_command("State?"), vec!["State:A"]);
    }

    #[test]
    fn test_engine_dual_side2_reference() {
        let mut engine = AtmegaEngine::new(EngineConfig::for_reference("058013", 2));
        assert_eq!(engine.handle_command("Side?"), vec!["Side:50"]);
        assert_eq!(engine.handle_command("Reference?"), vec!["Reference:058013"]);
    }

    #[test]
    fn test_charge_cycle_and_current_ramp() {
        let mut engine = AtmegaEngine::new(EngineConfig::for_reference("058001", 1));

        // 1. Vehicle plugged
        let frames_plug = engine.event_plug();
        assert!(frames_plug.contains(&"State:B".to_string()));
        assert_eq!(engine.state.state, 'B');
        assert_eq!(engine.state.cp_volts, 9.0);

        // 2. Vehicle authorizes charge
        let frames_charge = engine.event_charge();
        assert!(frames_charge.contains(&"State:C".to_string()));
        assert_eq!(engine.state.state, 'C');
        assert!(engine.state.contactor_closed);

        // 3. Ticks: current ramps up (simulate 10 seconds of 1s steps)
        let mut cci_received = false;
        for _ in 0..10 {
            if let Some(frame) = engine.tick_with_dt(1.0) {
                if frame.starts_with("CCI:") {
                    cci_received = true;
                }
            }
        }
        assert!(cci_received);
        assert!(engine.state.measured_current > 10.0);

        // 4. Current limit modulation (e.g. by EVCC)
        let frames_limit = engine.handle_command("CC:16");
        assert_eq!(frames_limit, vec!["CC:16"]);
        assert_eq!(engine.state.cc_limit, 16);

        // Ticks: current ramps down towards 16A
        for _ in 0..10 {
            engine.tick_with_dt(1.0);
        }
        assert!((engine.state.measured_current - 16.0).abs() <= 1.0);

        // 5. User pauses via front button / API (SBNOK)
        let frames_stop = engine.handle_command("SBNOK");
        assert_eq!(frames_stop, vec!["State:M"]);
        assert_eq!(engine.state.state, 'M');
        assert!(!engine.state.contactor_closed);
        assert_eq!(engine.state.measured_current, 0.0);

        // 6. User resumes (SBOK)
        let frames_resume = engine.handle_command("SBOK");
        assert_eq!(frames_resume, vec!["State:B", "State:C"]);
        assert_eq!(engine.state.state, 'C');

        // 7. Vehicle unplugged
        let frames_unplug = engine.event_unplug();
        assert!(frames_unplug.contains(&"State:A".to_string()));
        assert_eq!(engine.state.state, 'A');
        assert_eq!(engine.state.measured_current, 0.0);
    }
}
