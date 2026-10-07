use rumqttc::{AsyncClient, MqttOptions, QoS, Event, Incoming};
use std::time::Duration;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use greenup_driver::state::{AppState, BoardInfo, Telemetry, TicDetectionState};
use greenup_driver::logger;
use greenup_everest::{EverestAdapter, EverestBspEvent};

#[tokio::main]
async fn main() {
    // 1. Initialiser le driver série (emprunté de greenup-link)
    let (serial_tx, serial_rx) = mpsc::channel(32);
    let driver_state = Arc::new(AppState {
        telemetry: Mutex::new(Telemetry::default()),
        info: Mutex::new(BoardInfo::default()),
        serial_tx,
        tic_detection: Mutex::new(TicDetectionState::default()),
    });

    let state_for_serial = driver_state.clone();
    std::thread::spawn(move || {
        greenup_driver::serial::run_serial_loop(state_for_serial, serial_rx);
    });

    // Lancer la séquence d'init après 1 seconde
    let state_for_init = driver_state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(1)).await;
        logger::log("EVEREST", "🔄 Envoi séquence initialisation Legrand...");
        greenup_driver::serial::trigger_init_sequence(&state_for_init).await;
    });

    // 2. Initialiser l'Adaptateur EVerest
    let adapter = Arc::new(EverestAdapter::new(driver_state.clone()));
    
    let adapter_for_loop = adapter.clone();
    tokio::spawn(async move {
        adapter_for_loop.run_event_loop().await;
    });

    // 3. Connexion MQTT (Configurable via variable d'environnement)
    let mqtt_host = std::env::var("MQTT_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let mut mqttoptions = MqttOptions::new("greenup-everest-driver", &mqtt_host, 1883);
    mqttoptions.set_keep_alive(Duration::from_secs(5));
    
    let (client, mut connection) = AsyncClient::new(mqttoptions, 10);
    
    // Souscriptions aux ordres d'EVerest
    client.subscribe("everest/board_support/cmd/allow_power_on", QoS::AtMostOnce).await.unwrap();
    client.subscribe("everest/board_support/cmd/set_pwm", QoS::AtMostOnce).await.unwrap();
    client.subscribe("everest/board_support/cmd/reset", QoS::AtMostOnce).await.unwrap();

    logger::log("EVEREST", "🔌 Connecté au broker MQTT local (Port 1883)");

    // 4. Tâche d'émission (Legrand -> EVerest)
    let client_pub = client.clone();
    let adapter_pub = adapter.clone();
    tokio::spawn(async move {
        let mut evt_rx = adapter_pub.subscribe_events();
        let mut tel_rx = adapter_pub.subscribe_telemetry();
        
        loop {
            tokio::select! {
                Ok(evt) = evt_rx.recv() => {
                    let evt_str = match evt {
                        EverestBspEvent::Disconnected => "A",
                        EverestBspEvent::Connected => "B",
                        EverestBspEvent::Charging => "C",
                        EverestBspEvent::Error => "Error",
                        EverestBspEvent::Faulted => "Faulted",
                    };
                    logger::log("EVEREST", &format!("📤 Émission BspEvent: {}", evt_str));
                    let _ = client_pub.publish("everest/board_support/event", QoS::AtMostOnce, false, evt_str).await;
                }
                Ok(tel) = tel_rx.recv() => {
                    // Powermeter JSON
                    let payload = serde_json::json!({
                        "voltage_V": tel.voltage_v,
                        "current_A": tel.current_a,
                        "power_W": tel.power_w,
                        "energy_Wh": tel.energy_wh,
                    });
                    let _ = client_pub.publish("everest/powermeter/telemetry", QoS::AtMostOnce, false, payload.to_string()).await;
                }
            }
        }
    });

    // 5. Boucle de réception MQTT (EVerest -> Legrand)
    loop {
        match connection.poll().await {
            Ok(Event::Incoming(Incoming::Publish(p))) => {
                let topic = p.topic;
                let payload_str = String::from_utf8_lossy(&p.payload);
                
                match topic.as_str() {
                    "everest/board_support/cmd/allow_power_on" => {
                        let allow = payload_str.trim() == "true";
                        logger::log("EVEREST", &format!("📥 Commande allow_power_on: {}", allow));
                        let _ = adapter.allow_power_on(allow).await;
                    }
                    "everest/board_support/cmd/set_pwm" => {
                        if let Ok(duty) = payload_str.trim().parse::<f32>() {
                            logger::log("EVEREST", &format!("📥 Commande set_pwm: {}%", duty));
                            if let Err(e) = adapter.set_pwm(duty).await {
                                logger::log("EVEREST", &format!("⚠️ Erreur PWM: {}", e));
                            }
                        }
                    }
                    "everest/board_support/cmd/reset" => {
                        logger::log("EVEREST", "📥 Commande Reset Matériel");
                        let _ = adapter.hardware_reset().await;
                    }
                    _ => {}
                }
            }
            Ok(_) => {}
            Err(e) => {
                logger::log("EVEREST", &format!("⚠️ Erreur MQTT: {:?}", e));
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}
