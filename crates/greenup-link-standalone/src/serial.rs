use crate::state::SharedState;
use crate::logger;
use std::io::{Read, Write};
use std::time::Duration;
use tokio::sync::mpsc;
use greenup_protocol::{parse_line, Command, FunctioningMode, ProtocolEvent};

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
                let hello = Command::RaspberryPiModeOk;
                let _ = port.write_all(hello.encode().as_bytes());
                logger::log("SERIE_TX", &hello.as_frame());
                std::thread::sleep(Duration::from_millis(200));

                // Demande des infos de base avec délais pour ne pas saturer le buffer RX
                for cmd in Command::startup_queries() {
                    let _ = port.write_all(cmd.encode().as_bytes());
                    logger::log("SERIE_TX", &cmd.as_frame());
                    std::thread::sleep(Duration::from_millis(150));
                }

                // Séquence de détection TIC : on envoie TICTM:1 et on attend TICTestB:XXXX
                if let Ok(mut count) = state.tic_test_zero_count.lock() {
                    *count = 0;
                }
                if let Ok(mut tel) = state.telemetry.lock() {
                    tel.tic_mode = "detecting".to_string(); // Indicateur pour l'IHM
                }
                let tic = Command::SetTicTestMode(true);
                let _ = port.write_all(tic.encode().as_bytes());
                logger::log("SERIE_TX", &format!("{} (début détection TIC)", tic));
                std::thread::sleep(Duration::from_millis(150));

                let mut read_buf = [0u8; 1024];
                let mut line_buffer = String::new();

                loop {
                    // 1. Lire s'il y a des commandes HTTP en attente d'envoi vers la borne
                    while let Ok(cmd) = rx.try_recv() {
                        let trimmed = cmd.trim().to_string();
                        logger::log("SERIE_TX", &trimmed);
                        if let Err(e) = port.write_all(cmd.as_bytes()) {
                            logger::log("SERIE", &format!("⚠️ Erreur d'envoi: {:?}", e));
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
                            logger::log("SERIE", &format!("⚠️ Erreur de lecture : {:?}. Reconnexion...", e));
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
    logger::log("SERIE_RX", line);

    let event = parse_line(line);

    match event {
        ProtocolEvent::Ping => {
            logger::log("SERIE", "🤝 Ping matériel détecté, envoi de RaspberryPiModeOK");
            let _ = state.serial_tx.try_send(Command::RaspberryPiModeOk.encode());
        }
        ProtocolEvent::SoftwareVersion(v) => {
            if let Ok(mut info) = state.info.lock() { info.software_version = v; }
        }
        ProtocolEvent::HardwareVersion(v) => {
            if let Ok(mut info) = state.info.lock() { info.hardware_version = v; }
        }
        ProtocolEvent::SerialNumber(v) => {
            if let Ok(mut info) = state.info.lock() { info.serial_number = v; }
        }
        ProtocolEvent::Reference(v) => {
            if let Ok(mut info) = state.info.lock() {
                let spec = greenup_protocol::ModelSpec::from_reference(&v);
                if !spec.is_known {
                    logger::log("SERIE", &format!("⚠️ Référence inconnue ({}), on assume un modèle de base (Mono 4.6kW)", v));
                } else {
                    logger::log("SERIE", &format!("ℹ️ Modèle identifié : {}", spec.name));
                }
                info.reference = v;
                info.spec = Some(spec);
            }
        }
        ProtocolEvent::WeekYearProduction(v) => {
            if let Ok(mut info) = state.info.lock() { info.week_year_production = v; }
        }
        ProtocolEvent::BluetoothState(enabled) => {
            if let Ok(mut info) = state.info.lock() { info.bluetooth_enabled = Some(enabled); }
        }
        ProtocolEvent::StateChange(state_val) => {
            if let Ok(mut tel) = state.telemetry.lock() {
                if (state_val == "D" || state_val == "E") && (tel.state != "D" && tel.state != "E") {
                    tel.energy = 0.0;
                    tel.last_power_update = Some(std::time::Instant::now());
                }
                tel.state = state_val.clone();
                if state_val == "A" || state_val == "L" {
                    tel.charge_complete = false;
                }
            }
        }
        ProtocolEvent::ErrorChange(e) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.error_code = e; }
        }
        ProtocolEvent::ChargeComplete => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.charge_complete = true; }
        }
        ProtocolEvent::FmMode(fm_val) => {
            if FunctioningMode::from_code(&fm_val) != Some(FunctioningMode::DirectCharge) {
                logger::log("SERIE", &format!("⚠️ Mode FM détecté = {}, forçage en FM:1", fm_val));
                let _ = state.serial_tx.try_send(
                    Command::SetFunctioningMode(FunctioningMode::DirectCharge).encode(),
                );
            }
        }
        ProtocolEvent::TicTestBaud(val) => {
            if val != "0" {
                logger::log("SERIE", &format!("🔌 TIC détecté : {} baud", val));
                if let Ok(mut tel) = state.telemetry.lock() { tel.tic_mode = val; }
                // On ne renvoie plus TICTM:0, on laisse la borne remonter ses trames de test
            } else {
                if let Ok(mut count) = state.tic_test_zero_count.lock() {
                    *count += 1;
                    if *count >= TICTM_MAX_ZEROS {
                        logger::log("SERIE", "🔌 TIC non détecté (absent)");
                        if let Ok(mut tel) = state.telemetry.lock() { tel.tic_mode = "0".to_string(); }
                        // Idem, on ne force plus l'arrêt du mode test pour l'instant
                    }
                }
            }
        }
        ProtocolEvent::TicTestInit => {}
        ProtocolEvent::Voltage(v) => {
            if let Ok(mut tel) = state.telemetry.lock() {
                tel.voltage = v;
                tel.power = tel.voltage * tel.current;
            }
        }
        ProtocolEvent::Current(c) => {
            if let Ok(mut tel) = state.telemetry.lock() {
                let previous_power = tel.power;
                tel.current = c;
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
        ProtocolEvent::LimitAmps(limit) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.limit_amps = limit; }
        }
        ProtocolEvent::Energy(e) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.energy = e; }
        }
        ProtocolEvent::Frequency(f) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.frequency = f; }
        }
        ProtocolEvent::Unknown(_) => {}
    }
}
