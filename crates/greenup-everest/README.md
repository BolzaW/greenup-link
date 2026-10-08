# EVerest Module - GreenUp Driver (`greenup-everest`)

The goal of this module is to expose the Legrand Green'Up Premium charging station as a standardized `evse_board_support` (BSP) and `powermeter` module for the EVerest framework.

## Objective

EVerest expects to drive a "dumb" charge controller (sending PWM pulses, closing the contactor, reading CP voltage). However, the Legrand board embeds its own state machine (it manages its own safety, contactor, and off-peak hours logic).

The `greenup-everest` crate acts as a **Facade / Adapter**:
1. **Events (BSP Events)**: Translates the deduced logical state (`Disconnected_A`, `Charging_C`...) into recognized EVerest events.
2. **Commands (Allow Power On)**: Maps the EVerest charge authorization command (`allow_power_on`) to software presses on the START/STOP buttons (`SBOK` / `SBNOK`), which cleanly pause the Legrand machine.
3. **Current Limit (Set PWM)**: Converts the duty cycle requested by the EVerest energy manager (`duty_cycle_pct`) into hardware Amperes for the board (`CC:XX`).
4. **Telemetry (Powermeter)**: Publishes power, voltage, and energy in a format digestible by EVerest.

## Final Integration (MQTT Bridge)

Currently, this crate works as a **standalone executable (MQTT Daemon)**. It connects to a local MQTT broker and acts as a translation bridge between the Legrand serial port and the outside world.

### MQTT API (Standard Topics)

To integrate this station into EVerest, use EVerest's generic MQTT modules and configure them to subscribe/publish to the following topics:

#### Emission (Legrand ➔ EVerest)
*   **Topic:** `everest/board_support/event`
    *   **Payload (Text):** `A`, `B`, `C`, `Error`, `Faulted`
    *   **Description:** IEC 61851 machine state (cable plugged, charging in progress, etc.).
*   **Topic:** `everest/powermeter/telemetry`
    *   **Payload (JSON):** `{"voltage_V": 230.0, "current_A": 16.0, "power_W": 3680.0, "energy_Wh": 15000.0}`
    *   **Description:** Reporting of energy counters and instantaneous power.

#### Reception (EVerest ➔ Legrand)
*   **Topic:** `everest/board_support/cmd/allow_power_on`
    *   **Payload (Text):** `true` or `false`
    *   **Description:** Authorizes or suspends charging (simulates a Start/Stop button press).
*   **Topic:** `everest/board_support/cmd/set_pwm`
    *   **Payload (Text):** Duty cycle in percentage (e.g., `26.6` for 16A).
    *   **Description:** Modifies the current setpoint. The Legrand board does not manage actual PWM; this percentage is converted to Amperes (`Amps = PWM * 0.6`) and sent to the board. Limit is bounded between 7A and 32A.
*   **Topic:** `everest/board_support/cmd/reset`
    *   **Payload (Text):** Any value (e.g., `1`).
    *   **Description:** Triggers a hardware reboot (ATmega Reset).

### EVerest Configuration

In your EVerest `config.json` file, you will need to configure the "Generic MQTT" modules to match inputs/outputs with these topics.

## Limitations and Warnings (Hardware Quirks)

Due to the proprietary nature of the Legrand board firmware, certain physical constraints are imposed on this module and on EVerest:

1. **The Powermeter is largely falsified (emulated):**
   The Legrand station does not have a real (MID certified) energy meter. The only value actually measured by the hardware is the current (`current_A`). The voltage (`voltage_V`) always returns a fixed theoretical value of 230V. Consequently, the power (`power_W`) and cumulative energy (`energy_Wh`) values exposed to EVerest are pure mathematical deductions (P = U × I), based on the assumption of perfect voltage.

2. **Auto-start on plug (Direct jump to state C):**
   The Legrand board (ATmega) is physically designed to operate exclusively in "Plug & Charge". When a vehicle is plugged in (transition from state A to B), **the Legrand firmware automatically forces its internal state to `SB:1` and immediately starts charging (state C)**, *regardless* of whether a stop order (`SBNOK`) was previously sent to it!
   Our MQTT adapter does not perform magic to intercept this: it merely reports the transition to state C to EVerest. If EVerest is configured with an authorization profile (e.g., requiring an RFID badge before charging), EVerest will notice that the charge started without its permission and will react by immediately sending a cutoff command (`allow_power_on(false)` which triggers an `SBNOK`).
   It is therefore perfectly normal to hear a relay "click" and observe a one-second micro-charge upon plugging in, while EVerest reacts to suspend the charge (`State:M`). This is an unavoidable behavior linked to the Legrand firmware's persistence in starting the charge on its own.

## Adapter Architecture

```rust
pub struct EverestAdapter {
    // Listens to translated events for EVerest
    pub fn subscribe_events(&self) -> broadcast::Receiver<EverestBspEvent>;
    pub fn subscribe_telemetry(&self) -> broadcast::Receiver<EverestTelemetry>;

    // Commands translated from EVerest to Legrand
    pub async fn allow_power_on(&self, allow: bool) -> Result<(), String>;
    pub async fn set_pwm(&self, duty_cycle_pct: f32) -> Result<(), String>;
}
```
