use crate::state::SharedState;
use crate::logger;
use std::io::{Read, Write};
use std::time::Duration;
use tokio::sync::mpsc;
use greenup_protocol::{parse_line, Command, FunctioningMode, ProtocolEvent};

const TICTM_MAX_ZEROS: u32 = 5;

pub async fn trigger_init_sequence(state: &SharedState) {
    let hello = Command::RaspberryPiModeOk;
    let _ = state.serial_tx.send(hello.encode()).await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    for cmd in Command::startup_queries() {
        let _ = state.serial_tx.send(cmd.encode()).await;
        tokio::time::sleep(Duration::from_millis(150)).await;
    }


}

pub fn send_init_sequence_sync(port: &mut Box<dyn serialport::SerialPort>, _state: &SharedState) {
    let hello = Command::RaspberryPiModeOk;
    let _ = port.write_all(hello.encode().as_bytes());
    logger::log("SERIE_TX", &hello.as_frame());
    std::thread::sleep(Duration::from_millis(200));

    for cmd in Command::startup_queries() {
        let _ = port.write_all(cmd.encode().as_bytes());
        logger::log("SERIE_TX", &cmd.as_frame());
        std::thread::sleep(Duration::from_millis(150));
    }


}

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
                if let Ok(mut tic) = state.tic_detection.lock() { tic.is_active = true; tic.zero_count = 0; }
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

use crate::telemetry::Telemetry;

fn update_iec_state(tel: &mut Telemetry) {
    if tel.greenup_state == "V" {
        tel.iec_state = Some("Faulted_F".to_string());
        return;
    }
    if tel.greenup_state == "R" || tel.greenup_state == "X" {
        tel.iec_state = Some("Error_E".to_string());
        return;
    }

    if let Some(false) = tel.t2c_enabled {
        match tel.cp_voltage {
            Some(12) => tel.iec_state = Some("Disconnected_A".to_string()),
            Some(9) => tel.iec_state = Some("Connected_B".to_string()),
            Some(6) => tel.iec_state = Some("Charging_C".to_string()),
            _ => {}
        }
        return;
    }

    match tel.greenup_state.as_str() {
        "A" | "L" => tel.iec_state = Some("Disconnected_A".to_string()),
        "B" | "C" | "I" | "W" | "M" => tel.iec_state = Some("Connected_B".to_string()),
        "D" | "E" => tel.iec_state = Some("Charging_C".to_string()),
        _ => {}
    }
}

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
                let spec = crate::hardware_specs::HardwareCapabilities::from_reference(&v);
                if !spec.is_known {
                    logger::log("SERIE", &format!("⚠️ Référence inconnue ({}), on assume un modèle de base (Mono 4.6kW)", v));
                } else {
                    logger::log("SERIE", &format!("ℹ️ Modèle identifié : {}", spec.name));
                }
                info.reference = v;
                info.capabilities = Some(spec);
            }
        }
        ProtocolEvent::WeekYearProduction(v) => {
            if let Ok(mut info) = state.info.lock() { info.week_year_production = v; }
        }
        ProtocolEvent::BluetoothState(enabled) => {
            if let Ok(mut info) = state.info.lock() { info.bluetooth_enabled = Some(enabled); }
        }
        ProtocolEvent::StateChange(state_val) => {
            if state_val == "V" {
                logger::log("SYS", "!!! ALERTE CRITIQUE : COUPURE DE COURANT DÉTECTÉE (State:V) - EXTINCTION IMMINENTE !!!");
            }
            if let Ok(mut tel) = state.telemetry.lock() {
                if (state_val == "D" || state_val == "E") && (tel.greenup_state != "D" && tel.greenup_state != "E") {
                    tel.energy = 0.0;
                    tel.last_power_update = Some(std::time::Instant::now());
                }
                tel.greenup_state = state_val.clone(); update_iec_state(&mut tel);
                if state_val == "A" || state_val == "L" {
                    tel.charge_complete = false;
                }
            }
        }
        ProtocolEvent::ErrorChange(e) => {
            if e == "0012" {
                logger::log("SYS", "!!! ALERTE CRITIQUE : DÉFAUT SOUS-TENSION (E:0012) - EXTINCTION IMMINENTE !!!");
            }
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
            let mut should_stop_test = false;
            if let Ok(mut tic) = state.tic_detection.lock() {
                if tic.is_active {
                    if val != "0" {
                        should_stop_test = true;
                        tic.is_active = false;
                    } else {
                        tic.zero_count += 1;
                        if tic.zero_count >= TICTM_MAX_ZEROS {
                            should_stop_test = true;
                            tic.is_active = false;
                        }
                    }
                }
            }

            if val != "0" {
                logger::log("SERIE", &format!("🔌 TIC détecté : {} baud", val));
                if let Ok(mut tel) = state.telemetry.lock() { tel.tic_mode = val; }
            } else if should_stop_test {
                logger::log("SERIE", "🔌 TIC non détecté (absent)");
                if let Ok(mut tel) = state.telemetry.lock() { tel.tic_mode = "0".to_string(); }
            }

            if should_stop_test {
                logger::log("SERIE", "Fin de la détection automatique du TIC, envoi de TICTM:0");
                let _ = state.serial_tx.try_send(Command::SetTicTestMode(false).encode());
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
                    if tel.greenup_state == "D" || tel.greenup_state == "E" {
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
        ProtocolEvent::EliotLimitAmps(limit) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.eliot_limit_amps = Some(limit); }
        }
        ProtocolEvent::CpVoltage(v) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.cp_voltage = Some(v); update_iec_state(&mut tel); }
        }
        ProtocolEvent::T2CEnabled(v) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.t2c_enabled = Some(v); update_iec_state(&mut tel); }
        }
        ProtocolEvent::SbState(v) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.sb_state = Some(v); update_iec_state(&mut tel); }
        }
        ProtocolEvent::Energy(e) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.energy = e; }
        }
        ProtocolEvent::Frequency(f) => {
            if let Ok(mut tel) = state.telemetry.lock() { tel.frequency = f; }
        }
        ProtocolEvent::CommandNotUnderstood(cmd) => {
            logger::log("SERIE", &format!("⚠️ Commande non reconnue par la borne : {}", cmd));
        }
        ProtocolEvent::Unknown(_) => {}
    }
}
