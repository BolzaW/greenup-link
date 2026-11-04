use greenup_driver::SharedDriver;
use greenup_protocol::{Command, MAX_CURRENT_AMPS, MIN_CURRENT_AMPS};
use tokio::sync::broadcast;

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
    driver: SharedDriver,
    event_tx: broadcast::Sender<EverestBspEvent>,
    telemetry_tx: broadcast::Sender<EverestTelemetry>,
}

fn raw_state_to_iec(raw: &greenup_driver::AtmegaRawState) -> String {
    let state = raw.state.as_deref().unwrap_or("A");
    if state == "U" || state == "V" {
        "F".to_string()
    } else if matches!(state, "R" | "S" | "T" | "X") {
        "E".to_string()
    } else if raw.t2c == Some(false) {
        match raw.cp {
            Some(12) => "A".to_string(),
            Some(9) => "B".to_string(),
            Some(6) => "C".to_string(),
            _ => "A".to_string(),
        }
    } else {
        match state {
            "A" | "L" | "DISCONNECTED" => "A".to_string(),
            "B" | "C" | "F" | "I" | "J" | "K" | "M" | "N" | "O" | "P" | "W" => "B".to_string(),
            "D" | "E" | "G" | "H" => "C".to_string(),
            _ => "A".to_string(),
        }
    }
}

impl EverestAdapter {
    /// Initializes the EVerest adapter by wrapping the low-level GreenUp driver.
    pub fn new(driver: SharedDriver) -> Self {
        let (event_tx, _) = broadcast::channel(16);
        let (telemetry_tx, _) = broadcast::channel(16);
        Self {
            driver,
            event_tx,
            telemetry_tx,
        }
    }

    /// Asynchronous task that subscribes to the low-level driver watch channel and emits EVerest events.
    pub async fn run_event_loop(&self) {
        let mut rx = self.driver.subscribe();
        let mut last_iec = String::new();
        let mut cumulative_energy_wh = 0.0_f32;
        let mut last_power_w = 0.0_f32;
        let mut last_update: Option<std::time::Instant> = None;

        loop {
            if rx.changed().await.is_err() {
                break;
            }
            let snap = rx.borrow().clone();
            let current_iec = raw_state_to_iec(&snap.atmega);
            let is_charging = matches!(
                snap.atmega.state.as_deref(),
                Some("D") | Some("E") | Some("G") | Some("H")
            );
            let voltage_v = snap.atmega.volt.unwrap_or(230.0);
            let current_a = if is_charging {
                snap.atmega.cci.unwrap_or(0.0)
            } else {
                0.0
            };
            let power_w = voltage_v * current_a;

            let now = std::time::Instant::now();
            if is_charging {
                if let Some(prev_time) = last_update {
                    let dt_hours = prev_time.elapsed().as_secs_f32() / 3600.0;
                    cumulative_energy_wh += ((last_power_w + power_w) / 2.0) * dt_hours;
                }
                last_update = Some(now);
            } else {
                last_update = None;
                if current_iec == "A" {
                    cumulative_energy_wh = 0.0;
                }
            }
            last_power_w = power_w;

            let _ = self.telemetry_tx.send(EverestTelemetry {
                voltage_v,
                current_a,
                power_w,
                energy_wh: snap.atmega.ener.unwrap_or(cumulative_energy_wh),
            });

            if current_iec != last_iec {
                let evt = match current_iec.as_str() {
                    "A" => Some(EverestBspEvent::Disconnected),
                    "B" => Some(EverestBspEvent::Connected),
                    "C" => Some(EverestBspEvent::Charging),
                    "E" => Some(EverestBspEvent::Error),
                    "F" => Some(EverestBspEvent::Faulted),
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
        let cmd = Command::SetStartButton(allow);
        self.driver
            .send_command(cmd)
            .await
            .map_err(|_| "TX send error".to_string())
    }

    /// (EVerest) set_pwm: Translates a PWM duty cycle to Amperes for Legrand
    pub async fn set_pwm(&self, duty_cycle_pct: f32) -> Result<(), String> {
        let mut amps = (duty_cycle_pct * 0.6).round() as u8;
        if amps < MIN_CURRENT_AMPS {
            amps = MIN_CURRENT_AMPS;
        } else if amps > MAX_CURRENT_AMPS {
            amps = MAX_CURRENT_AMPS;
        }

        if let Ok(cmd) = Command::set_current_limit(amps) {
            self.driver
                .send_command(cmd)
                .await
                .map_err(|_| "TX send error".to_string())
        } else {
            Err("Current setpoint out of absolute tolerance".to_string())
        }
    }

    /// Requests a hardware reset
    pub async fn hardware_reset(&self) -> Result<(), String> {
        self.driver
            .send_command(Command::Reset)
            .await
            .map_err(|_| "TX send error".to_string())
    }
}
