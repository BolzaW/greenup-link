use tokio::sync::broadcast;
use greenup_driver::SharedState;
use greenup_protocol::{Command, MIN_CURRENT_AMPS, MAX_CURRENT_AMPS};

/// Événements traduits pour EVerest (interface evse_board_support)
#[derive(Debug, Clone, PartialEq)]
pub enum EverestBspEvent {
    /// Le câble est déconnecté (State A)
    Disconnected,
    /// Le câble est connecté mais la charge n'a pas commencé (State B)
    Connected,
    /// La charge est en cours (State C/D)
    Charging,
    /// Erreur matérielle (State E)
    Error,
    /// Défaut critique (State F / V)
    Faulted,
}

/// Télémétrie standardisée pour le module EVerest Powermeter
#[derive(Debug, Clone)]
pub struct EverestTelemetry {
    pub voltage_v: f32,
    pub current_a: f32,
    pub power_w: f32,
    pub energy_wh: f32,
}

pub struct EverestAdapter {
    driver_state: SharedState,
    event_tx: broadcast::Sender<EverestBspEvent>,
    telemetry_tx: broadcast::Sender<EverestTelemetry>,
}

impl EverestAdapter {
    /// Initialise l'adaptateur EVerest en encapsulant le driver GreenUp existant.
    pub fn new(driver_state: SharedState) -> Self {
        let (event_tx, _) = broadcast::channel(16);
        let (telemetry_tx, _) = broadcast::channel(16);
        Self {
            driver_state,
            event_tx,
            telemetry_tx,
        }
    }

    /// Tâche asynchrone qui poll le driver Legrand et émet les événements EVerest
    pub async fn run_event_loop(&self) {
        let mut last_iec = String::new();
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

            let (current_iec, telemetry_update) = {
                if let Ok(tel) = self.driver_state.telemetry.lock() {
                    let iec = tel.iec_state.clone().unwrap_or_else(|| "Unknown".to_string());
                    let tele = EverestTelemetry {
                        voltage_v: tel.voltage,
                        current_a: tel.current,
                        power_w: tel.power,
                        energy_wh: tel.energy * 1000.0, // Everest attend souvent des Wh
                    };
                    (iec, tele)
                } else {
                    continue;
                }
            };

            // Émission de la télémétrie
            let _ = self.telemetry_tx.send(telemetry_update);

            // Détection des changements d'état IEC pour générer les BspEvent
            if current_iec != last_iec {
                let evt = match current_iec.as_str() {
                    "Disconnected_A" => Some(EverestBspEvent::Disconnected),
                    "Connected_B" => Some(EverestBspEvent::Connected),
                    "Charging_C" => Some(EverestBspEvent::Charging),
                    "Error_E" => Some(EverestBspEvent::Error),
                    "Faulted_F" => Some(EverestBspEvent::Faulted),
                    _ => None,
                };

                if let Some(e) = evt {
                    let _ = self.event_tx.send(e);
                }
                last_iec = current_iec;
            }
        }
    }

    /// S'abonne aux changements d'états (BSP Events)
    pub fn subscribe_events(&self) -> broadcast::Receiver<EverestBspEvent> {
        self.event_tx.subscribe()
    }

    /// S'abonne à la télémétrie (Powermeter)
    pub fn subscribe_telemetry(&self) -> broadcast::Receiver<EverestTelemetry> {
        self.telemetry_tx.subscribe()
    }

    // =========================================================================
    // COMMANDES EVEREST -> LEGRAND
    // =========================================================================

    /// (EVerest) allow_power_on: Autorise ou bloque la charge
    pub async fn allow_power_on(&self, allow: bool) -> Result<(), String> {
        // En EVerest, `allow_power_on(true)` autorise la charge (State C).
        // On traduit cela par l'appui sur le bouton Start (SBOK) ou Stop (SBNOK).
        let cmd = Command::SetStartButton(allow);
        self.driver_state
            .serial_tx
            .send(cmd)
            .await
            .map_err(|_| "Erreur d'envoi TX".to_string())
    }

    /// (EVerest) set_pwm: Traduit un rapport cyclique (duty cycle PWM) en Ampères pour Legrand
    pub async fn set_pwm(&self, duty_cycle_pct: f32) -> Result<(), String> {
        // En IEC 61851, un duty cycle de 10% à 85% correspond à Ampères = duty_cycle * 0.6
        // Everest envoie un pourcentage (ex: 26.66% = 16A).
        let mut amps = (duty_cycle_pct * 0.6).round() as u8;
        
        // Sécurité matérielle Legrand
        if amps < MIN_CURRENT_AMPS {
            amps = MIN_CURRENT_AMPS;
        } else if amps > MAX_CURRENT_AMPS {
            amps = MAX_CURRENT_AMPS;
        }

        if let Ok(cmd) = Command::set_current_limit(amps) {
            self.driver_state
                .serial_tx
                .send(cmd)
                .await
                .map_err(|_| "Erreur d'envoi TX".to_string())
        } else {
            Err("Consigne de courant hors tolérance absolue".to_string())
        }
    }

    /// Demande un redémarrage matériel (si supporté par la couche supérieure)
    pub async fn hardware_reset(&self) -> Result<(), String> {
        self.driver_state
            .serial_tx
            .send(Command::Reset)
            .await
            .map_err(|_| "Erreur d'envoi TX".to_string())
    }
}
