use clap::Parser;
#[cfg(unix)]
use greenup_simulator::cli::SimulatorCli;
#[cfg(unix)]
use greenup_simulator::engine::EngineConfig;
#[cfg(unix)]
use greenup_simulator::pty::PtyPair;
#[cfg(unix)]
use greenup_simulator::worker::AtmegaWorker;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "High-fidelity hardware simulator for Legrand Green'Up Premium charging stations"
)]
struct Args {
    /// Enable dual charge point mode (Point 1 on /tmp/ttyUSB0, Point 2 on /tmp/ttyUSB1)
    #[arg(long, default_value_t = false)]
    dual: bool,

    /// Official Legrand station commercial reference (e.g. 058001, 058013)
    #[arg(long, default_value = "058013")]
    model: String,

    /// Symlink path for Point 1 (Right socket)
    #[arg(long, default_value = "/tmp/ttyUSB0")]
    port1: PathBuf,

    /// Symlink path for Point 2 (Left socket)
    #[arg(long, default_value = "/tmp/ttyUSB1")]
    port2: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    println!("\x1b[1;32m===========================================================\x1b[0m");
    println!("\x1b[1;32m        GREEN'UP HARDWARE SIMULATOR (ATmega2560)           \x1b[0m");
    println!("  Model: \x1b[1;33m{}\x1b[0m", args.model);
    println!(
        "  Mode:  \x1b[1;36m{}\x1b[0m",
        if args.dual {
            "Dual-Point (Point 1 Right + Point 2 Left)"
        } else {
            "Single-Point (Point 1 Right)"
        }
    );
    println!("\x1b[1;32m===========================================================\x1b[0m");

    #[cfg(unix)]
    {
        // 1. Initialiser le Point 1
        let pty1 = PtyPair::new(Some(&args.port1))?;
        println!(
            "🔌 Point 1 PTY ready: \x1b[1m{}\x1b[0m -> {:?}",
            pty1.slave_path.display(),
            args.port1
        );
        let cfg1 = EngineConfig::for_reference(&args.model, 1);
        let (worker1, _handle1) = AtmegaWorker::spawn(pty1, cfg1)?;
        println!(
            "⚡ Point 1 Engine active (Side: 1, Max: {}A)",
            worker1.engine.lock().unwrap().config.max_hardware_amps
        );

        // 2. Initialiser le Point 2 (si activé)
        let worker2 = if args.dual {
            let pty2 = PtyPair::new(Some(&args.port2))?;
            println!(
                "🔌 Point 2 PTY ready: \x1b[1m{}\x1b[0m -> {:?}",
                pty2.slave_path.display(),
                args.port2
            );
            let cfg2 = EngineConfig::for_reference(&args.model, 2);
            let (w2, _handle2) = AtmegaWorker::spawn(pty2, cfg2)?;
            println!(
                "⚡ Point 2 Engine active (Side: 2, Max: {}A)",
                w2.engine.lock().unwrap().config.max_hardware_amps
            );
            Some(w2)
        } else {
            None
        };

        // 3. Gestionnaire CLI interactif
        let cli = SimulatorCli::new(worker1, worker2);

        // Capture de Ctrl+C en parallèle du prompt interactif
        tokio::select! {
            _ = cli.run_interactive_loop() => {},
            _ = tokio::signal::ctrl_c() => {
                println!("\n🛑 Interrupted by Ctrl+C.");
            }
        }

        println!("🛑 Cleaning up simulator and removing virtual serial symlinks...");
    }

    #[cfg(not(unix))]
    {
        eprintln!("❌ PTY simulation is only supported on Linux / WSL environments.");
    }

    Ok(())
}
