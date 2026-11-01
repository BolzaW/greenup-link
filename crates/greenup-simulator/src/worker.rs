use crate::engine::{AtmegaEngine, EngineConfig, EngineState};
#[cfg(unix)]
use crate::pty::PtyPair;
use std::io::Write;
#[cfg(unix)]
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum UserEvent {
    Plug,
    Charge,
    PauseCar,
    ResumeCar,
    Unplug,
    CarLimit(u8),
    Fault(String),
    ClearFault,
}

#[derive(Debug, Clone)]
pub struct WorkerSnapshot {
    pub side: u8,
    pub reference: String,
    pub state: EngineState,
}

#[derive(Clone)]
pub struct AtmegaWorker {
    pub side: u8,
    pub engine: Arc<Mutex<AtmegaEngine>>,
    event_tx: mpsc::Sender<UserEvent>,
}

impl AtmegaWorker {
    #[cfg(unix)]
    pub fn spawn(pty: PtyPair, config: EngineConfig) -> std::io::Result<(Self, tokio::task::JoinHandle<()>)> {
        let side = config.side;
        let master_read = pty.try_clone_master()?;
        let mut master_write = std::fs::File::from(pty.try_clone_master()?);

        let engine = Arc::new(Mutex::new(AtmegaEngine::new(config)));
        let engine_clone = engine.clone();

        let (tx_out, mut rx_out) = mpsc::channel::<String>(128);
        let (event_tx, mut event_rx) = mpsc::channel::<UserEvent>(32);
        let (rx_in, mut rx_cmd) = mpsc::channel::<String>(128);

        // 1. Thread dédié à la lecture bloquante du PTY maître
        std::thread::spawn(move || {
            let mut file = std::fs::File::from(master_read);
            let mut buf = Vec::with_capacity(256);
            let mut byte = [0u8; 1];
            loop {
                match file.read(&mut byte) {
                    Ok(0) => break, // EOF / esclave fermé
                    Ok(_) => {
                        if byte[0] == b'\r' || byte[0] == b'\n' {
                            if !buf.is_empty() {
                                let line = String::from_utf8_lossy(&buf).trim().to_string();
                                buf.clear();
                                if !line.is_empty() {
                                    if rx_in.blocking_send(line).is_err() {
                                        break;
                                    }
                                }
                            }
                        } else {
                            buf.push(byte[0]);
                            if buf.len() >= 256 {
                                buf.clear();
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // 2. Tâche d'écriture asynchrone sur le PTY maître
        let side_color = if side == 1 { "\x1b[32m" } else { "\x1b[35m" };
        tokio::spawn(async move {
            while let Some(frame) = rx_out.recv().await {
                let wire = format!("{}\r\n", frame);
                if master_write.write_all(wire.as_bytes()).is_err() || master_write.flush().is_err() {
                    break;
                }
            }
        });

        // 3. Tâche principale d'orchestration (dispatch commandes, ticks 1Hz et événements utilisateur)
        let tx_out_clone = tx_out.clone();
        let handle = tokio::spawn(async move {
            let _pty_guard = pty;
            let mut interval = tokio::time::interval(Duration::from_millis(1000));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    // Commande reçue depuis Green'Up Link
                    Some(cmd) = rx_cmd.recv() => {
                        let replies = {
                            let mut eng = engine_clone.lock().unwrap();
                            eng.handle_command(&cmd)
                        };

                        println!("{}[Point {} RX]\x1b[0m < {}", side_color, side, cmd);

                        for reply in replies {
                            println!("{}[Point {} TX]\x1b[0m > {}", side_color, side, reply);
                            let _ = tx_out_clone.send(reply).await;
                        }
                    }

                    // Événement déclenché depuis la console CLI
                    Some(event) = event_rx.recv() => {
                        let frames = {
                            let mut eng = engine_clone.lock().unwrap();
                            match event {
                                UserEvent::Plug => eng.event_plug(),
                                UserEvent::Charge => eng.event_charge(),
                                UserEvent::PauseCar => eng.event_pause_car(),
                                UserEvent::ResumeCar => eng.event_resume_car(),
                                UserEvent::Unplug => eng.event_unplug(),
                                UserEvent::CarLimit(amps) => {
                                    eng.set_car_limit(amps);
                                    vec![]
                                }
                                UserEvent::Fault(code) => eng.event_fault(&code),
                                UserEvent::ClearFault => eng.event_clear_fault(),
                            }
                        };

                        for frame in frames {
                            println!("{}[Point {} EVENT]\x1b[0m > {}", side_color, side, frame);
                            let _ = tx_out_clone.send(frame).await;
                        }
                    }

                    // Tick périodique (1 seconde) : physique électrique & émission CCI
                    _ = interval.tick() => {
                        let spontaneous_opt = {
                            let mut eng = engine_clone.lock().unwrap();
                            eng.tick()
                        };

                        if let Some(frame) = spontaneous_opt {
                            println!("{}[Point {} TELEM]\x1b[0m > {}", side_color, side, frame);
                            let _ = tx_out_clone.send(frame).await;
                        }
                    }
                }
            }
        });

        Ok((
            Self {
                side,
                engine,
                event_tx,
            },
            handle,
        ))
    }

    pub async fn trigger(&self, event: UserEvent) -> Result<(), mpsc::error::SendError<UserEvent>> {
        self.event_tx.send(event).await
    }

    pub fn snapshot(&self) -> WorkerSnapshot {
        let eng = self.engine.lock().unwrap();
        WorkerSnapshot {
            side: self.side,
            reference: eng.config.reference.clone(),
            state: eng.state.clone(),
        }
    }
}
