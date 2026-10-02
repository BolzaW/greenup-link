use crate::logger;
use crate::state::SharedState;
use std::io::{Read, Write};
use std::time::Duration;
use tokio::sync::mpsc;

/// Nombre max de TICTestB:0 avant de conclure que le TIC est absent
const TICTM_MAX_ZEROS: u32 = 5;

pub fn run_serial_loop(state: SharedState, mut rx: mpsc::Receiver<String>) {
    let port_name = "/dev/ttyUSB0";
    let baud_rate = 115200;

    loop {
        logger::log("SERIE", &format!("Tentative d'ouverture de {}...", port_name));
        
        match serialport::new(port_name, baud_rate)
            .timeout(Duration::from_millis(50))
            .open()
        {
            Ok(mut port) => {
                logger::log("SERIE", "✅ Port ouvert. Envoi de la séquence d'initialisation...");
                
                // Trame magique
                let _ = port.write_all(b"RaspberryPiModeOK\r");
                logger::log("SERIE_TX", "RaspberryPiModeOK");
                std::thread::sleep(Duration::from_millis(200));

                // Demande des infos de base avec délais pour ne pas saturer le buffer RX
                let init_cmds = [
                    "SoftwareVersion?", "HardwareVersion?", "SerialNumber?",
                    "Reference?", "WeekYearProduction?", "State?", "FM?", "CC?", "E?", "BT?",
                ];
                for cmd in &init_cmds {
                    let full = format!("{}\r", cmd);
                    let _ = port.write_all(full.as_bytes());
                    logger::log("SERIE_TX", cmd);
                    std::thread::sleep(Duration::from_millis(150));
                }

                // Séquence de détection TIC : on envoie TICTM:1 et on attend TICTestB:XXXX
                if let Ok(mut count) = state.tic_test_zero_count.lock() {
                    *count = 0;
                }
                if let Ok(mut tel) = state.telemetry.lock() {
                    tel.tic_mode = "detecting".to_string(); // Indicateur pour l'IHM
                }
                let _ = port.write_all(b"TICTM:1\r");
                logger::log("SERIE_TX", "TICTM:1 (début détection TIC)");
                std::thread::sleep(Duration::from_millis(150));

                let mut read_buf = [0u8; 1024];
                let mut line_buffer = String::new();

                loop {
                    // 1. Lire s'il y a des commandes HTTP en attente d'envoi vers la borne
                    while let Ok(cmd) = rx.try_recv() {
                        let trimmed = cmd.trim().to_string();
                        logger::log("SERIE_TX", &trimmed);
                        if let Err(e) = port.write_all(cmd.as_bytes()) {
                            logger::log("SERIE", &format!("❌ Erreur d'envoi: {:?}", e));
                        }
                    }

                    // 2. Écouter la borne
                    match port.read(&mut read_buf) {
                        Ok(bytes_read) if bytes_read > 0 => {
                            if let Ok(text) = std::str::from_utf8(&read_buf[..bytes_read]) {
                                line_buffer.push_str(text);
                                
                                while let Some(pos) = line_buffer.find('\n') {
                                    let line = line_buffer[..pos].trim_end_matches('\r').to_string();
                                    line_buffer = line_buffer[pos + 1..].to_string();
                                    
                                    if !line.is_empty() {
                                        parse_incoming_line(&line, &state);
                                    }
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {}
                        Err(e) => {
                            logger::log("SERIE", &format!("❌ Erreur de lecture : {:?}. Reconnexion...", e));
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                logger::log("SERIE", &format!("Impossible d'ouvrir {}: {:?}. Réessai dans 5s...", port_name, e));
                std::thread::sleep(Duration::from_secs(5));
            }
        }
    }
}

// Fonction qui analyse chaque ligne venant de la carte et met à jour la mémoire partagée
fn parse_incoming_line(line: &str, state: &SharedState) {
    // Log de chaque ligne reçue
    logger::log("SERIE_RX", line);

    // --- RÉPONSE AUTOMATIQUE (WATCHDOG/PING) ---
    if line.starts_with("RaspberryPi?") {
        logger::log("SERIE", "🤝 Ping matériel détecté, envoi de RaspberryPiModeOK");
        let _ = state.serial_tx.try_send("RaspberryPiModeOK\r".to_string());
        return;
    }

    // --- MISE À JOUR DES INFOS FIXES ---
    if line.starts_with("SoftwareVersion:") {
        if let Ok(mut info) = state.info.lock() {
            info.software_version = line.replace("SoftwareVersion:", "");
        }
    } else if line.starts_with("HardwareVersion:") {
        if let Ok(mut info) = state.info.lock() {
            info.hardware_version = line.replace("HardwareVersion:", "");
        }
    } else if line.starts_with("SerialNumber:") {
        if let Ok(mut info) = state.info.lock() {
            info.serial_number = line.replace("SerialNumber:", "");
        }
    } else if line.starts_with("Reference:") {
        if let Ok(mut info) = state.info.lock() {
            info.reference = line.replace("Reference:", "");
        }
    } else if line.starts_with("WeekYearProduction:") {
        if let Ok(mut info) = state.info.lock() {
            info.week_year_production = line.replace("WeekYearProduction:", "");
        }
    } else if line.starts_with("BT:") {
        let val = line.replace("BT:", "");
        if let Ok(mut info) = state.info.lock() {
            info.bluetooth_enabled = Some(val == "1");
        }
    } 
    // --- MISE À JOUR DE LA TÉLÉMÉTRIE ---
    else if line.starts_with("State:") {
        if let Ok(mut tel) = state.telemetry.lock() {
            let state_val = line.replace("State:", "");
            
            // Si on démarre une nouvelle session de charge, on réinitialise le compteur d'énergie
            if (state_val == "D" || state_val == "E") && (tel.state != "D" && tel.state != "E") {
                tel.energy = 0.0;
                tel.last_power_update = Some(std::time::Instant::now());
            }
            
            tel.state = state_val.clone();
            if state_val == "A" || state_val == "L" {
                tel.charge_complete = false;
            }
        }
    } else if line.starts_with("E:") {
        if let Ok(mut tel) = state.telemetry.lock() {
            tel.error_code = line.replace("E:", "");
        }
    } else if line.starts_with("FCS:") {
        if let Ok(mut tel) = state.telemetry.lock() {
            tel.charge_complete = true;
        }
    } else if line.starts_with("FM:") {
        let fm_val = line.replace("FM:", "");
        if fm_val != "1" {
            logger::log("SERIE", &format!("⚠️ Mode FM détecté = {}, forçage en FM:1", fm_val));
            let _ = state.serial_tx.try_send("FM:1\r".to_string());
        }
    } else if line.starts_with("TICTestB:") {
        let val = line.replace("TICTestB:", "");
        if val != "0" {
            logger::log("SERIE", &format!("🔌 TIC détecté : {} baud", val));
            if let Ok(mut tel) = state.telemetry.lock() {
                tel.tic_mode = val;
            }
            let _ = state.serial_tx.try_send("TICTM:0\r".to_string());
        } else {
            if let Ok(mut count) = state.tic_test_zero_count.lock() {
                *count += 1;
                if *count >= TICTM_MAX_ZEROS {
                    logger::log("SERIE", "🔌 TIC non détecté (absent)");
                    if let Ok(mut tel) = state.telemetry.lock() {
                        tel.tic_mode = "0".to_string();
                    }
                    let _ = state.serial_tx.try_send("TICTM:0\r".to_string());
                }
            }
        }
    } else if line.starts_with("TICTestC:") {
        // Just logged, nothing to do.
    } else if line.starts_with("Volt:") {
        if let Ok(val) = line.replace("Volt:", "").parse::<f32>() {
            if let Ok(mut tel) = state.telemetry.lock() {
                tel.voltage = val;
                tel.power = tel.voltage * tel.current;
            }
        }
    } else if line.starts_with("CCI:") {
        let clean_val = line.replace("CCI:", "");
        if let Ok(val) = clean_val.parse::<f32>() {
            if let Ok(mut tel) = state.telemetry.lock() {
                let previous_power = tel.power;
                tel.current = val;
                tel.power = tel.voltage * tel.current;
                
                let now = std::time::Instant::now();
                if let Some(last_time) = tel.last_power_update {
                    let elapsed_hours = last_time.elapsed().as_secs_f32() / 3600.0;
                    if tel.state == "D" || tel.state == "E" {
                        let energy_wh = ((previous_power + tel.power) / 2.0) * elapsed_hours;
                        tel.energy += energy_wh;
                    }
                }
                tel.last_power_update = Some(now);
            }
        }
    } else if line.starts_with("CC:") {
        let clean_val = line.replace("CC:", "");
        if let Ok(val) = clean_val.parse::<u32>() {
            if let Ok(mut tel) = state.telemetry.lock() {
                tel.limit_amps = val;
            }
        }
    } else if line.starts_with("Ener:") {
        if let Ok(val) = line.replace("Ener:", "").parse::<f32>() {
            if let Ok(mut tel) = state.telemetry.lock() {
                tel.energy = val;
            }
        }
    } else if line.starts_with("Freq:") {
        if let Ok(val) = line.replace("Freq:", "").parse::<f32>() {
            if let Ok(mut tel) = state.telemetry.lock() {
                tel.frequency = val;
            }
        }
    }
}
