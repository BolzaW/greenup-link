use crate::worker::{AtmegaWorker, UserEvent, WorkerSnapshot};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct SimulatorCli {
    point1: AtmegaWorker,
    point2: Option<AtmegaWorker>,
}

impl SimulatorCli {
    pub fn new(point1: AtmegaWorker, point2: Option<AtmegaWorker>) -> Self {
        Self { point1, point2 }
    }

    pub async fn run_interactive_loop(&self) {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();

        println!("\x1b[1;36mType 'help' for available commands, 'status' for dashboard, or 'quit' to exit.\x1b[0m\n");

        print_prompt();

        while let Ok(Some(line)) = reader.next_line().await {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                let cmd = parts[0].to_lowercase();

                match cmd.as_str() {
                    "help" | "?" => print_help(),
                    "status" => self.print_status(),
                    "plug" => {
                        let side = parse_side(parts.get(1));
                        self.dispatch(side, UserEvent::Plug).await;
                    }
                    "charge" | "start" => {
                        let side = parse_side(parts.get(1));
                        self.dispatch(side, UserEvent::Charge).await;
                    }
                    "pause" | "pause-car" => {
                        let side = parse_side(parts.get(1));
                        self.dispatch(side, UserEvent::PauseCar).await;
                    }
                    "resume" | "resume-car" => {
                        let side = parse_side(parts.get(1));
                        self.dispatch(side, UserEvent::ResumeCar).await;
                    }
                    "unplug" | "stop" => {
                        let side = parse_side(parts.get(1));
                        self.dispatch(side, UserEvent::Unplug).await;
                    }
                    "car-limit" => {
                        let side = parse_side(parts.get(1));
                        let amps: u8 = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(16);
                        println!("🚘 Setting car onboard charger limit on Point {} to {}A", side, amps);
                        self.dispatch(side, UserEvent::CarLimit(amps)).await;
                    }
                    "fault" => {
                        let side = parse_side(parts.get(1));
                        let code = parts.get(2).unwrap_or(&"E:0010").to_string();
                        println!("⚠️ Injecting fault {} on Point {}", code, side);
                        self.dispatch(side, UserEvent::Fault(code)).await;
                    }
                    "clear-fault" => {
                        let side = parse_side(parts.get(1));
                        println!("✅ Clearing fault on Point {}", side);
                        self.dispatch(side, UserEvent::ClearFault).await;
                    }
                    "scenario" => {
                        let sc_name = parts.get(1).unwrap_or(&"full").to_lowercase();
                        let side = parse_side(parts.get(2));
                        self.run_scenario(&sc_name, side).await;
                    }
                    "quit" | "exit" => {
                        println!("🛑 Exiting simulator...");
                        break;
                    }
                    other => {
                        println!("\x1b[31mUnknown command: '{}'. Type 'help' for instructions.\x1b[0m", other);
                    }
                }
            }
            print_prompt();
        }
    }

    async fn dispatch(&self, side: u8, event: UserEvent) {
        if side == 2 {
            if let Some(ref p2) = self.point2 {
                let _ = p2.trigger(event).await;
            } else {
                println!("\x1b[31mError: Point 2 is not enabled (single-point mode). Run with --dual.\x1b[0m");
            }
        } else {
            let _ = self.point1.trigger(event).await;
        }
    }

    fn print_status(&self) {
        println!("\n\x1b[1;37m==================== SIMULATOR HARDWARE DASHBOARD ====================\x1b[0m");
        print_point_status(&self.point1.snapshot());
        if let Some(ref p2) = self.point2 {
            print_point_status(&p2.snapshot());
        }
        println!("\x1b[1;37m======================================================================\x1b[0m\n");
    }

    async fn run_scenario(&self, name: &str, side: u8) {
        match name {
            "full" => {
                println!("\x1b[1;34m▶️ Running Scenario 'full' on Point {} (60s realistic charging cycle)...\x1b[0m", side);
                println!("  [Step 1] Plug cable...");
                self.dispatch(side, UserEvent::Plug).await;
                tokio::time::sleep(Duration::from_secs(2)).await;

                println!("  [Step 2] Authorize charging (vehicle requests power)...");
                self.dispatch(side, UserEvent::Charge).await;
                tokio::time::sleep(Duration::from_secs(10)).await;

                println!("  [Step 3] Vehicle pauses charging...");
                self.dispatch(side, UserEvent::PauseCar).await;
                tokio::time::sleep(Duration::from_secs(2)).await;

                println!("  [Step 4] Unplug cable...");
                self.dispatch(side, UserEvent::Unplug).await;
                println!("\x1b[1;32m✅ Scenario 'full' completed on Point {}\x1b[0m", side);
            }
            "evcc" => {
                println!("\x1b[1;34m▶️ Running Scenario 'evcc' on Point {} (Current modulation test)...\x1b[0m", side);
                self.dispatch(side, UserEvent::Plug).await;
                tokio::time::sleep(Duration::from_secs(1)).await;
                self.dispatch(side, UserEvent::Charge).await;
                tokio::time::sleep(Duration::from_secs(5)).await;

                println!("  [EVCC] Car limit set to 10A...");
                self.dispatch(side, UserEvent::CarLimit(10)).await;
                tokio::time::sleep(Duration::from_secs(5)).await;

                println!("  [EVCC] Car limit set to 16A...");
                self.dispatch(side, UserEvent::CarLimit(16)).await;
                tokio::time::sleep(Duration::from_secs(5)).await;

                println!("  [EVCC] Car limit set to 32A...");
                self.dispatch(side, UserEvent::CarLimit(32)).await;
                tokio::time::sleep(Duration::from_secs(5)).await;

                self.dispatch(side, UserEvent::Unplug).await;
                println!("\x1b[1;32m✅ Scenario 'evcc' completed on Point {}\x1b[0m", side);
            }
            other => {
                println!("\x1b[31mUnknown scenario: '{}'. Available: 'full', 'evcc'.\x1b[0m", other);
            }
        }
    }
}

fn print_prompt() {
    print!("\x1b[1;32mgreenup-sim>\x1b[0m ");
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn parse_side(arg: Option<&&str>) -> u8 {
    arg.and_then(|s| s.parse::<u8>().ok()).unwrap_or(1)
}

fn print_point_status(snap: &WorkerSnapshot) {
    let side_color = if snap.side == 1 { "\x1b[1;32m" } else { "\x1b[1;35m" };
    let contactor_str = if snap.state.contactor_closed {
        "\x1b[32mCLOSED [ON]\x1b[0m"
    } else {
        "\x1b[31mOPEN [OFF]\x1b[0m"
    };

    println!(
        "{}Point {} (Side: {}, Ref: {})\x1b[0m",
        side_color, snap.side, snap.side, snap.reference
    );
    println!(
        "  State: \x1b[1mState:{}\x1b[0m | CP: {:.1}V | PP: {}A | Contactor: {}",
        snap.state.state, snap.state.cp_volts, snap.state.pp_amps, contactor_str
    );
    println!(
        "  Current: \x1b[1;33m{:.2} A\x1b[0m (Limit: {}A, Car: {}A) | Voltage: {:.1} V | Energy: {:.3} kWh",
        snap.state.measured_current, snap.state.cc_limit, snap.state.car_max_amps, snap.state.voltage, snap.state.energy_kwh
    );
    println!(
        "  Status: Error: \x1b[1m{}\x1b[0m | FM: {} | FM2: {} | Derogation: {} | T2C Auth: {}",
        snap.state.error_code, snap.state.functioning_mode, snap.state.external_signal_fm2, snap.state.derogation, snap.state.t2c_authorized
    );
    println!();
}

fn print_help() {
    println!("\x1b[1;37mAvailable Simulator Commands:\x1b[0m");
    println!("  \x1b[32mplug [side]\x1b[0m             - Connect EV cable (State B, CP=9V, PP=32A)");
    println!("  \x1b[32mcharge [side]\x1b[0m           - Vehicle authorizes charge (State C, CP=6V, contactor closes, current ramps up)");
    println!("  \x1b[32mpause [side]\x1b[0m            - Vehicle suspends charge (State B, CP=9V, contactor opens)");
    println!("  \x1b[32mresume [side]\x1b[0m           - Vehicle resumes charge (State C)");
    println!("  \x1b[32munplug [side]\x1b[0m           - Disconnect EV cable (State L -> State A, CP=12V)");
    println!("  \x1b[32mcar-limit [side] <amps>\x1b[0m - Set car onboard charger current limit (e.g. 16)");
    println!("  \x1b[32mfault [side] [code]\x1b[0m     - Inject hardware fault (e.g. E:0010 Overcurrent)");
    println!("  \x1b[32mclear-fault [side]\x1b[0m      - Clear fault and return to normal state");
    println!("  \x1b[32mstatus\x1b[0m                  - Display dashboard of all simulated charge points");
    println!("  \x1b[32mscenario <name> [side]\x1b[0m  - Run automated sequence ('full', 'evcc')");
    println!("  \x1b[32mhelp\x1b[0m                    - Show this help message");
    println!("  \x1b[32mquit\x1b[0m                    - Terminate simulator and clean up symlinks\n");
}
