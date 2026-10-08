use tokio::sync::broadcast;
use greenup_driver::SharedState;
use greenup_protocol::{Command, MIN_CURRENT_AMPS, MAX_CURRENT_AMPS};

/// Translated events for EVerest (evse_board_support interface)
#[derive(Debug, Clone, PartialEq)]
pub enum EverestBspEvent {
    /// Cable is disconnected (State A)
    Disconnected,
    /// Cable is connected but charging has not started (State B)
    Connected,
    /// Charging in progress (State C/D)
    Charging,
    /// Hardware error (State E)
    Error,
    /// Critical fault (State F / V)
    Faulted,
}

/// Standardized telemetry for the EVerest Powermeter module
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
    /// Initializes the EVerest adapter by wrapping the existing GreenUp driver.
    pub fn new(driver_state: SharedState) -> Self {
        let (event_tx, _) = broadcast::channel(16);
        let (telemetry_tx, _) = broadcast::channel(16);
        Self {
            driver_state,
            event_tx,
            telemetry_tx,
        }
    }

    /// Asynchronous task that polls the Legrand driver and emits EVerest events
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
                        energy_wh: tel.energy * 1000.0, // Everest often expects Wh
                    };
                    (iec, tele)
                } else {
                    continue;
                }
            };

            // Emit telemetry
            let _ = self.telemetry_tx.send(telemetry_update);

            // Detect IEC state changes to generate BspEvents
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

    /// Subscribes to state changes (BSP Events)
    pub fn subscribe_events(&self) -> broadcast::Receiver<EverestBspEvent> {
        self.event_tx.subscribe()
    }

    /// Subscribes to telemetry (Powermeter)
    pub fn subscribe_telemetry(&self) -> broadcast::Receiver<EverestTelemetry> {
        self.telemetry_tx.subscribe()
    }

    // =========================================================================
    // EVEREST -> LEGRAND COMMANDS
    // =========================================================================

    /// (EVerest) allow_power_on: Allows or blocks charging
    pub async fn allow_power_on(&self, allow: bool) -> Result<(), String> {
        // In EVerest, `allow_power_on(true)` allows charging (State C).
        // We translate this to pressing the Start (SBOK) or Stop (SBNOK) button.
        let cmd = Command::SetStartButton(allow);
        self.driver_state
            .serial_tx
            .send(cmd)
            .await
            .map_err(|_| "TX send error".to_string())
    }

    /// (EVerest) set_pwm: Translates a PWM duty cycle to Amperes for Legrand
    pub async fn set_pwm(&self, duty_cycle_pct: f32) -> Result<(), String> {
        // In IEC 61851, a 10% to 85% duty cycle corresponds to Amperes = duty_cycle * 0.6
        // Everest sends a percentage (e.g. 26.66% = 16A).
        let mut amps = (duty_cycle_pct * 0.6).round() as u8;
        
        // Legrand hardware safety
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
                .map_err(|_| "TX send error".to_string())
        } else {
            Err("Current setpoint out of absolute tolerance".to_string())
        }
    }

    /// Requests a hardware reset (if supported by the upper layer)
    pub async fn hardware_reset(&self) -> Result<(), String> {
        self.driver_state
            .serial_tx
            .send(Command::Reset)
            .await
            .map_err(|_| "TX send error".to_string())
    }
}
