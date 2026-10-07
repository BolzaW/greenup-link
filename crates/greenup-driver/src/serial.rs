use crate::logger;
use crate::state::SharedState;
use greenup_protocol::{commands::FunctioningMode, Command, parser::{parse_line, ProtocolEvent}};
use serialport::SerialPort;
use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use tokio::sync::mpsc;

const TICTM_MAX_ZEROS: u32 = 5;


pub async fn trigger_init_sequence(state: &SharedState) {
    for cmd in Command::startup_queries() {
        let _ = state.serial_tx.send(cmd).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

pub enum TicStartError {
    /// La borne n'est pas en `State:A` (contient l'état Legrand courant).
    NotInStateA(String),
    /// Le canal vers le port série est fermé.
    Serial,
}

/// Lance la détection TIC (`TICTM:1`), UNIQUEMENT si l'état Legrand est `A`.
///
/// Garde-fou centralisé : si la borne n'est pas en `State:A`, la fonction ne fait
/// strictement rien (aucune trame envoyée, aucun état modifié). Le driver se charge
/// ensuite d'envoyer `TICTM:0` à la fin de la détection.
pub async fn start_tic_detection(state: &SharedState) -> Result<(), TicStartError> {
    let current_state = match state.telemetry.lock() {
        Ok(tel) => tel.greenup_state.clone(),
        Err(_) => String::from("Unknown"),
    };

    if current_state != "A" {
        logger::log("SERIE", &format!("⛔ Détection TIC ignorée : borne non libre (State: {})", current_state));
        return Err(TicStartError::NotInStateA(current_state));
    }

    if let Ok(mut tic) = state.tic_detection.lock() { tic.is_active = true; tic.zero_count = 0; }
    if let Ok(mut tel) = state.telemetry.lock() { tel.tic_mode = "detecting".to_string(); }
    state
        .serial_tx
        .send(Command::SetTicTestMode(true))
        .await
        .map_err(|_| TicStartError::Serial)
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

pub fn run_serial_loop(state: SharedState, mut rx_channel: tokio::sync::mpsc::Receiver<Command>) {
    let mut port = serialport::new("/dev/ttyUSB0", 115_200)
        .timeout(Duration::from_millis(100))
        .open()
        .expect("Impossible d'ouvrir le port série /dev/ttyUSB0");

    let mut clone_port = port.try_clone().expect("Echec clone port serie");

    // Canal interne pour envoyer les lignes lues au thread TX pour acquittement
    let (internal_tx, internal_rx) = std::sync::mpsc::channel::<String>();

    let state_rx = state.clone();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(clone_port);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(n) if n > 0 => {
                    let clean_line = line.trim();
                    if clean_line.is_empty() { continue; }
                    
                    // On envoie une copie au TX pour le matching
                    let _ = internal_tx.send(clean_line.to_string());
                    
                    // On parse pour mettre a jour la telemetrie
                    parse_incoming_line(clean_line, &state_rx);
                }
                _ => {}
            }
        }
    });

    // Boucle TX (Thread courant)
    while let Some(cmd) = rx_channel.blocking_recv() {
        let encoded = cmd.encode();
        let expected_prefix = cmd.expected_rx_prefix();
        
        let mut retries = 1;
        let mut success = false;

        while retries >= 0 && !success {
            // Vider le canal des vieux messages
            while let Ok(_) = internal_rx.try_recv() {}

            logger::log("SERIE_TX", encoded.trim());
            if let Err(e) = port.write_all(encoded.as_bytes()) {
                logger::log("SERIE", &format!("Erreur d'ecriture serie: {}", e));
            }

            if let Some(prefix) = expected_prefix {
                let start_wait = std::time::Instant::now();
                let mut matched = false;
                let mut got_default = false;

                while start_wait.elapsed() < Duration::from_millis(1000) {
                    if let Ok(rx_line) = internal_rx.recv_timeout(Duration::from_millis(50)) {
                        if rx_line.starts_with(prefix) {
                            matched = true;
                            break;
                        } else if rx_line.starts_with("Default:") {
                            got_default = true;
                            break;
                        }
                    }
                }

                if matched {
                    success = true;
                } else {
                    if got_default {
                        logger::log("SERIE", &format!("❌ Commande {} refusee (Default), retry: {}", cmd.as_frame(), retries));
                    } else {
                        logger::log("SERIE", &format!("⏳ Commande {} timeout, retry: {}", cmd.as_frame(), retries));
                    }
                    retries -= 1;
                }
            } else {
                success = true; // Pas d'acquittement attendu
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
            let _ = state.serial_tx.try_send(Command::RaspberryPiModeOk);
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
                    Command::SetFunctioningMode(FunctioningMode::DirectCharge),
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
                let _ = state.serial_tx.try_send(Command::SetTicTestMode(false));
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
