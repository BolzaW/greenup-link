use clap::Parser;
#[cfg(unix)]
use greenup_simulator::engine::{AtmegaEngine, EngineConfig};
#[cfg(unix)]
use greenup_simulator::pty::PtyPair;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "High-fidelity hardware simulator for Legrand Green'Up Premium charging stations")]
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

    println!("===========================================================");
    println!("        GREEN'UP HARDWARE SIMULATOR (ATmega2560)           ");
    println!("  Model: {}", args.model);
    println!("  Mode:  {}", if args.dual { "Dual-Point (Point 1 + Point 2)" } else { "Single-Point (Point 1)" });
    println!("===========================================================");

    #[cfg(unix)]
    {
        let pty1 = PtyPair::new(Some(&args.port1))?;
        println!("🔌 Point 1 PTY initialized: {} -> {:?}", pty1.slave_path.display(), args.port1);

        let _pty2 = if args.dual {
            let p2 = PtyPair::new(Some(&args.port2))?;
            println!("🔌 Point 2 PTY initialized: {} -> {:?}", p2.slave_path.display(), args.port2);
            Some(p2)
        } else {
            None
        };

        let engine1 = AtmegaEngine::new(EngineConfig::for_reference(&args.model, 1));
        println!("⚡ Engine 1 ready (Side: 1, Max: {}A)", engine1.config.max_hardware_amps);

        if args.dual {
            let engine2 = AtmegaEngine::new(EngineConfig::for_reference(&args.model, 2));
            println!("⚡ Engine 2 ready (Side: 2, Max: {}A)", engine2.config.max_hardware_amps);
        }

        println!("\n✅ Simulator running. Press Ctrl+C to terminate and clean up symlinks.");
        tokio::signal::ctrl_c().await?;
        println!("\n🛑 Shutting down simulator and removing symlinks...");
    }

    #[cfg(not(unix))]
    {
        eprintln!("❌ PTY simulation is only supported on Linux / WSL environments.");
    }

    Ok(())
}
