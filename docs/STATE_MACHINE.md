# Legrand Charging Station State Machine (Green'Up Premium)

This document details the operation of the proprietary state machine in the Legrand ATmega firmware, its translation into standardized IEC 61851 and OCPP 1.6-J states by Green'Up Link, and the mechanisms used to control charging sessions reliably.

The states returned by the `State:` serial command are not simple IEC 61851 electrical states, but a behavioral finite-state machine (FSM).

> [!IMPORTANT]
> **Hardware Autonomy & Electrical Safety:**  
> Whatever happens to Green'Up Link (host system crash, Raspberry Pi reboot, USB cable disconnection, process restart, etc.), **the Legrand ATmega power board remains 100% autonomous and in full control of its internal state machine and physical electrical protections** (hardware RCD/differential protection, contactor safety interlocks, overcurrent monitoring, temperature safety, and TIC load-shedding).  
> The only consequence of losing Green'Up Link is the temporary loss of remote supervision and control; an ongoing charging session will simply run its course safely using the last current setpoint and authorization state received by the ATmega.

---

## 1. Full List of Proprietary States (`State:X`)

The semantics of these states were established through reverse-engineering of the power board firmware and cross-referenced with the station's internal control logic:

| State | Hardware Interpretation | OCPP Mapping (ATmega Internal Semantics) |
|---|---|---|
| **A** | Idle, no vehicle connected | Available |
| **B** | **T2S** socket occupied, waiting for authorization / external signal (held here under `FM2:1`, Linky Peak Hours, or `CAUNOK`) | SuspendedEVSE |
| **C** | T2S → charge suspended by vehicle (waiting for EV demand) | SuspendedEV |
| **D** | T2S → **charging in progress** (contact conditioning active: `FM2:1..3`) | Charging |
| **E** | T2S → **charging in progress** (direct mode: `FM2:0`) | Charging |
| **F** | **Domestic 2P+T** socket occupied (micro-switch closed), waiting for authorization / suspended | SuspendedEVSE |
| **G** | Domestic 2P+T → **charging in progress** (contact conditioning active: `FM2:1..3`) | Charging |
| **H** | Domestic 2P+T → **charging in progress** (direct mode: `FM2:0`) | Charging |
| **I** | T2S → charge suspended by vehicle (variant) | SuspendedEV |
| **J** | T2S → charge suspended by station | SuspendedEVSE |
| **K** | Charge suspended by station | SuspendedEVSE |
| **L** | **Vehicle unplugged** / end of session (transient) | Disconnected |
| **M** | **Local stop** (paused via Stop Button `SBNOK` or front button) | SuspendedEVSE |
| **N** | Suspended by station (T2S socket) | SuspendedEVSE |
| **O** | Suspended by station (Domestic socket) | SuspendedEVSE |
| **P** | Local stop in progress | SuspendedEVSE |
| **R** | **Fault** (socket lock, overload, contactor) | Faulted |
| **S** | **Fault** (contactor, Control Pilot) | Faulted |
| **T** | **Fault** (Control Pilot short-circuit) | Faulted |
| **U** | **Power loss** | Faulted |
| **V** | **Fatal power loss** (emitted with `E:0012` before shutdown) | Faulted |
| **W** | Transient stopping state emitted during deceleration / shutdown sequence (PWM set to 0A) | Preparing |
| **X** | ATmega microcontroller reboot in progress | Faulted |
| **Y** | Deep **sleep mode** (command `Slp:1`) | Unavailable |
| **Z** | Factory initialization / production mode | Unavailable |

---

## 2. State Sequencing

```text
                        +-----------------------------------+
                        |                                   |
        Slp:1           v                                   |
   Y <---------------  [A]  Idle / Available                |
   |                   / | \                                |
   | Slp:0            /  |  \                               |
   +-----------------+   |   +--------------------+         |
                     |   |                        |         |
              T2S    v   v DOM                    |         |
                    [B]  [F]  <--------+          |         |
                   / | \  / | \        |          |         |
                  /  |  \/  |  \       |          |         |
                 /   |  /\  |   \      |          |         |
                v    v v  v v    v     |          |         |
              [L]  [C]  [G]  [H]  [R]  |          |         |
               |   /|\    \   /        |          |         |
               |  / | \    \ /         |          |         |
               | v  v  v    v          |          |         |
               |[D] [E] [N] [B]--------+          |         |
               | | \ / |                          |         |
               | |  X  |                          |         |
               | v v v v                          |         |
               |[I]<->[J]                         |         |
               |  \    /                          |         |
               |   v  v                           |         |
               +-->[L]--> [R] --------------------+---------+
                                                            |
      [K] --> [R] --> [A]      [M] --> [P] --> [M]          |
      [O] --> [A]              [P] --> [R] --> [A] ---------+
```

---

## 3. Standardized IEC 61851 & OCPP State Mapping in Green'Up Link

Green'Up Link abstracts the 24 proprietary Legrand states into clean, standardized **IEC 61851** electrical states (`A`, `B`, `C`, `E`, `F`) and **OCPP 1.6-J ChargePointStatus** values (`Available`, `Preparing`, `Charging`, `SuspendedEV`, `SuspendedEVSE`, `Faulted`, `Unavailable`).

> [!IMPORTANT]
> **Independent Software Reconstruction vs `OCPPS?`**:  
> The ATmega's `OCPPS?` query command does return an OCPP-equivalent status corresponding to the current Legrand state. However, Green'Up Link deliberately **does not rely on `OCPPS?`**:
> 1. **Polled vs. Spontaneous:** Unlike `State:X`, which the ATmega broadcasts spontaneously upon every state transition, `OCPPS?` is never emitted autonomously. Relying on it would require continuously polling the serial bus after every telemetry event, adding unnecessary latency and bus contention.
> 2. **High-Level Control & Evolution:** Querying `OCPPS?` would tightly couple Green'Up Link to Legrand's proprietary firmware mapping choices (whether justified or quirky). By reconstructing the OCPP state directly from spontaneous telemetry (`State:X`), error registers, and active software pause flags, Green'Up Link maintains full control and adaptability for future integrations.

### 3.1 Mapping Table (Implemented in `crates/greenup-link/src/telemetry.rs`)

| Legrand `State` | IEC 61851 | OCPP 1.6-J Status | Vehicle Connected | Charging | Description |
| :---: | :---: | :---: | :---: | :---: | :--- |
| **`A`** | **`A`** | `Available` | `false` | `false` | Station idle, no vehicle connected. |
| **`L`** | **`A`** | `Available` | `false` | `false` | Vehicle unplugged (transient transition to `A`). |
| **`B`** | **`B`** | `Preparing` | `true` | `false` | Type 2 cable plugged, awaiting authorization / timer window. |
| **`F`** | **`B`** | `Preparing` | `true` | `false` | Domestic Schuko socket occupied, awaiting authorization. |
| **`C`** | **`B`** | `SuspendedEV` | `true` | `false` | Station ready to deliver power, but vehicle is not drawing current. |
| **`I`** | **`B`** | `SuspendedEV` | `true` | `false` | Charge paused or suspended by vehicle (e.g. battery full). |
| **`D`** | **`C`** | `Charging` | `true` | `true` | Active charge under Derogation (`DOK`). |
| **`E`** | **`C`** | `Charging` | `true` | `true` | Active charge under nominal mode. |
| **`G`** | **`C`** | `Charging` | `true` | `true` | Domestic Schuko socket active charge. |
| **`H`** | **`C`** | `Charging` | `true` | `true` | Domestic Schuko socket active charge (variant). |
| **`J`** | **`B`** | `SuspendedEVSE` | `true` | `false` | Charge suspended by station on Type 2 socket. |
| **`K`** | **`B`** | `SuspendedEVSE` | `true` | `false` | Charge suspended by station. |
| **`M`** | **`B`** | `SuspendedEVSE` | `true` | `false` | Station paused via Stop Button command (`SBNOK`). |
| **`N`** | **`B`** | `SuspendedEVSE` | `true` | `false` | Suspended by station (Type 2 socket). |
| **`O`** | **`B`** | `SuspendedEVSE` | `true` | `false` | Suspended by station (Domestic socket). |
| **`P`** | **`B`** | `SuspendedEVSE` | `true` | `false` | Local stop sequence in progress. |
| **`W`** | **`B`** | `Preparing` | `true` | `false` | Transient stopping state emitted during stop sequence. |
| **`R`** | **`E`** | `Faulted` | `true` | `false` | Recoverable fault (socket lock, overload, contactor). |
| **`S`** | **`E`** | `Faulted` | `true` | `false` | Contactor or Control Pilot fault. |
| **`T`** | **`E`** | `Faulted` | `true` | `false` | Control Pilot short-circuit. |
| **`X`** | **`E`** | `Faulted` | `true` | `false` | Microcontroller rebooting. |
| **`U`** | **`F`** | `Faulted` | `true` | `false` | Mains power loss. |
| **`V`** | **`F`** | `Faulted` | `true` | `false` | Fatal power loss before shutdown. |
| **`Y`** | **`A`** | `Unavailable` | `false` | `false` | Sleep mode (`Slp:1`). |
| **`Z`** | **`A`** | `Unavailable` | `false` | `false` | Production / Factory initialization mode. |

> [!NOTE]
> **Driver Internal Sentinel (`DISCONNECTED`):**  
> `DISCONNECTED` is not a frame emitted by the ATmega power board. It is an internal software sentinel injected by the Green'Up Link serial driver at server startup (before the first frame is read) or when the physical USB cable is unplugged. In this condition, Green'Up Link maps the connection to **IEC State `E`** (Error / Communication lost) and **OCPP Status `Unavailable`**, immediately terminates any active session with the reason `"Driver shutdown / Serial disconnected"`, and zeroes out power/current metering.

### 3.2 Hardware Socket Invalidation (`T2CNOK` / `2PCNOK`)

> [!WARNING]
> **State Machine Lockout via Socket Invalidation:**  
> When a socket is disabled via `T2CNOK` (Type 2 socket) or `2PCNOK` (Domestic Schuko socket), the ATmega **completely freezes the state machine for that socket in `State:A`**.  
> In this disabled state (`T2C:0` or `2PC:0`), the ATmega stops reacting to physical cable insertions, latching solenoids, and button presses.
> 
> A theoretical workaround to track cable presence while `T2C` is disabled would be to poll the raw analog Control Pilot ADC voltage using `CP?`:
> - `CP:12` (12V) $\rightarrow$ No vehicle connected (`IEC A`)
> - `CP:9` (9V) $\rightarrow$ Vehicle connected, ready (`IEC B`)
> - `CP:6` (6V) $\rightarrow$ Vehicle requesting charge (`IEC C`)
> 
> However, **this workaround only applies to the Type 2 socket** (domestic Schuko sockets have no Control Pilot, relying only on physical micro-switches that the ATmega completely ignores when `2PCNOK` is active). Furthermore, active polling of `CP?` is not implemented in Green'Up Link.  
> 
> **Green'Up Link design choice:** Hardware sockets are kept authorized (`T2COK` / `2PCOK`) so that the ATmega's native plug-detection engine remains active, and charge authorization is managed cleanly via the `FM2` and `D` software signals (see Section 5.1).

---

## 4. Tele-Information Client (TIC / Linky Smart Metering)

### 4.1 What is TIC? (Background for Non-French Users)
In France, all public utility electricity meters managed by Enedis (including both legacy electronic meters and modern "Linky" smart meters) feature an optical-isolated digital communication output called **TIC** (*Télé-Information Client*).

This interface continuously broadcasts instantaneous grid metrics over a two-wire twisted pair:
- Current contractual tariff period: **Peak Hours (HP - Heures Pleines)** vs. **Off-Peak Hours (HC - Heures Creuses)**.
- Total subscribed electrical power ($P_{sub}$ or $I_{sub}$, e.g. 6 kVA / 30A, 9 kVA / 45A, 12 kVA / 60A).
- Real-time total household current draw ($I_{inst}$).

### 4.2 Built-In Station Capabilities
The Legrand Green'Up Premium charging station incorporates a dedicated internal demodulator that can be directly wired to the utility meter's TIC terminals. When connected, the ATmega manages two functions autonomously:
1. **Off-Peak Charging Synchronization**: Automatically inhibits vehicle charging during Peak Hours (HP) and starts charging during Off-Peak Hours (HC).
2. **Autonomous Dynamic Load-Balancing (*Délestage*)**: The ATmega continuously monitors total home consumption. Every 10 seconds, it calculates the remaining headroom ($I_{available} = I_{sub} - I_{inst}$) and updates the internal current limit **`CCTIC`**. If other high-power household appliances turn on (heat pump, oven, water heater), the station dynamically throttles vehicle charging current down to prevent tripping the main utility breaker.

### 4.3 Protocol Compatibility: Historical vs. Standard TIC
French Linky meters can operate in two distinct transmission modes:
- **TIC Historique (1200 bauds, 7E1)**: The legacy protocol format dating back to the 1990s.
- **TIC Standard (9600 bauds, 7E1)**: The modernized protocol format with expanded telemetry fields.

> [!CAUTION]
> **TIC Standard is NOT Supported by the Firmware:**  
> Despite the presence of code routines inside the ATmega firmware intended to parse 9600-baud Standard frames, **the ATmega firmware does not work reliably with TIC Standard**. Demodulator framing and timing errors prevent proper decoding.  
> Legrand customer support officially states that **TIC Standard is unsupported and will not be fixed**.  
> If you connect a Linky meter directly to the station, **the Linky meter must be configured in Historical Mode (`TIC Historique` at 1200 bauds)**.

### 4.4 How TIC Modifies State Machine Behavior
Connecting an active TIC Historique signal directly alters the ATmega's internal finite-state machine:
1. **Forced `State:B` during Peak Hours**: When an EV is plugged in during Peak Hours (HP), the station **locks the state machine in `State:B`**, refusing to close the contactor until the meter broadcasts the Off-Peak (HC) tariff transition.
2. **Dynamic 10-Second Setpoint Cycling**: Every 10 seconds, the station recalculates `CCTIC`. The effective charging current `CC` is updated to $\min(\text{DIP}, \text{CCCa}, \text{CCEl}, \text{CCTIC})$.
3. **End-of-Charge Transitions**: When a charging session completes under TIC management, the ATmega may loop back to `State:B` instead of holding steadily in `State:I`.

### 4.5 Why Native Linky TIC Load-Shedding (*Délestage*) is Problematic (e.g. on 6 kVA Subscriptions)
Field analysis reveals major architectural flaws in the native ATmega TIC load-shedding implementation:
1. **The 0.85 Derating Coefficient**: The firmware systematically deducts a hardcoded 15% safety margin from the total subscribed capacity ($I_{max\_allowed} = I_{sub} \times 0.85$). On a standard 6 kVA single-phase contract (30 A), the station caps whole-house consumption at $25.5\text{ A}$ total!
2. **10-Second Control Loop Latency**: The firmware recalculates the PWM setpoint only once every 10 seconds. When a heavy domestic load (such as an induction hob, kettle, or space heater) suddenly starts, the station cannot ramp down charging fast enough to avoid an impending trip.
3. **Emergency Trip Guillotine & 10-Minute Penalty**: If available headroom drops below 8 A or the instantaneous overload reaches 125%, a fast-path watchdog aborts charging immediately without waiting for PWM ramping, opening the contactor directly. Worse, the station enforces a mandatory **10-minute timeout penalty** before allowing any retry!

### 4.6 Caution: Mixing Physical TIC with External Software EMS (EVCC / HA)
While running both the physical TIC link and an external energy management system (like EVCC or Home Assistant) is technically functional, **co-habitation is not recommended**:
- The ATmega's internal 10-second TIC load-balancing loop and EVCC's dynamic control loop (`CCEl:XX`) operate concurrently without coordination.
- Under fast solar fluctuations or large domestic load steps, having two independent controllers adjusting the current setpoint simultaneously can produce setpoint oscillations or unwanted charge interruptions.
- **Recommendation:** If you use EVCC for dynamic solar charging or home load-balancing, let EVCC handle all measurements and setpoint modulations via software, without wiring the physical TIC to the station.

---

## 5. Charge Control Strategies & State Machine Interventions

Green'Up Link implements two tiers of charge control to supervise charging reliably without running into firmware deadlock or infinite reboot loops:

### 5.1 Master Session Control: The `D` + `FM2` Combination (`enable` / `disable`)

The primary charge gatekeeper in Green'Up Link uses a combination of Derogation (`D`) and the Dry Contact conditioning sub-mode (`FM2`):

#### Understanding `FM2` as a 2-bit Forcing Mask
`FM2` is a 2-bit mask controlling dry contact conditioning:
- **`FM2:0` (`00`)**: Direct / Nominal mode (charges in `State:E` / `State:H`). Immediate charge upon vehicle plug-in.
- **`FM2:1` (`01`)**: Conditioned on Contact 1 (Off-Peak contact simulation / Peak-Hours lockout; charges in `State:D` / `State:G`).
  - Inhibits automatic charging on cable plug-in until Contact 1 closes or `DOK` is sent.
  - Setting `FM2:1` causes `FM?` to return `2`.
- **`FM2:2` (`10`)**: Conditioned on Contact 2 (not overridable by `DOK`).
- **`FM2:3` (`11`)**: Conditioned on Contact 1 AND Contact 2.

#### Understanding `D` (Dérogation)
Derogation (`DOK` / `DNOK`) is a software flag placed in logical OR with the physical Off-Peak dry contact (Contact 1) and the Linky TIC Off-Peak signal (`Contact_HC || TIC_HC || Derogation`).
- `DOK` immediately lifts a Linky Peak-Hours (HP) lockout or an `FM2:1` conditioning block.

#### To Start / Authorize Charge (`action: "enable"`):
1. Sends `T2COK`: Ensures the Type 2 socket state machine is awake and operational.
2. Sends `FM2:0`: Clears external simulated blocking signal (restores direct mode).
3. Sends `DOK`: Activates Derogation.

**Why this combination is bulletproof:**
- Clearing `FM2:0` and activating `DOK` transitions the station from `State:B` to active charge (**`State:E`**).
- **Overrides TIC Peak Hours:** If a physical TIC is wired, `DOK` overrides the Peak-Hours (HP) restriction, allowing immediate charging.
- **Preserves Autonomous Load-Balancing:** Physical TIC load-shedding remains active under Derogation; `CCTIC` will still throttle the charge if household consumption spikes.

#### To Stop / Block Charge (`action: "disable"`):
1. Sends `FM2:1`: Asserts Contact 1 conditioning (Peak-Hours simulation), blocking automatic charge initiation.
2. Sends `DNOK`: Releases the derogation flag.

**Why this combination is safe:**
- `FM2:1` holds the station in `State:B` without opening contactors under load.
- It prevents the physical TIC from automatically triggering a charge when an Off-Peak (HC) tariff window arrives.

---

### 5.2 Charge Authorization Control (`CAUOK` / `CAUNOK`) & Clean Cutoff

`CAU` stands for **Charge AUthorization**:
- **Clean Deceleration & Cutoff (`CAUNOK`)**: Ramps down charging current via deceleration state **`State:W`** (PWM to 0 A), opens the contactor off-load, and holds the vehicle plugged and locked in **`State:B`**. *(In `State:D`, `CAUNOK` requires `RaspberryPiModeOK` to be active).*
- **Restoring Authorization (`CAUOK`)**: Re-arms charge authorization.
- **Avoid `CAU?` Query**: `CAU?` compares the T2S and Domestic authorization flags; if they differ (which occurs during TIC load-shedding or single-socket sessions), `CAU?` resets both to `0` and **abruptly cuts off active charging**.

---

### 5.3 Deferred Start (`Dec:`) & Calendar Matrix

- **`Dec:<minutes>` (Deferred Start)**: Active **only in `FM:1`** (`FM2:0`), quantized in **3-hour increments** (`0`, `180`, `360`, `540` min; values > 540 are ignored).
  - **Expiration Behavior**: When the countdown expires, charging starts if authorized (still blocked if Linky TIC is in Peak Hours without `DOK`).
- **`Calendar?` (Matrix Display)**: Read-only console printout of a 7-day schedule. **Evaluated only in standalone `FM:4`**.

---

### 5.4 Front Panel Push Button & Asynchronous `EM:` Push Notifications

- **Short Press (< ~1 s)**: Toggles the session Start/Stop flag (`SB`), emitting `SBF:1` or `SBF:0` (and clearing any active `Dec:` timer on stop).
- **Long Press (> ~1 s)**: Toggles the hardware derogation state (equivalent to `DOK` / `DNOK`).
- **Continuous Hold (in `FM:1` plastic enclosure)**: Cycles through deferred start delays (`180` $\rightarrow$ `360` $\rightarrow$ `540` $\rightarrow$ `0` min) every 2 s and emits `Dec:<min>` on release.
- **Spontaneous `EM:` Notifications**:
  - Emitted asynchronously **only when `FM:2` (`FM2:1..3`) and `RaspberryPiModeOK` are active**:
  - **`EM:1XX` / `EM:0XX`**: Contact 2 state.
  - **`EM:X1X` / `EM:X0X`**: Contact 1 state.
  - **`EM:XX1` / `EM:XX0`**: Derogation state.

---

### 5.5 Domestic 2P+T Auxiliary Socket Behavior

- **Presence Detection (`DOM?`)**: Reads the socket insertion micro-switch $\rightarrow$ `DOM:0` (empty) or `DOM:1` (plugged).
- **Dedicated FSM States**:
  - **`State:F`**: Plug inserted, waiting / suspended.
  - **`State:H`**: Active charge at 16 A (`FM2:0`).
  - **`State:G`**: Active charge with contact conditioning (`FM2:1..3`).
- **Absolute T2S Priority**: Plugging a vehicle into the Type 2 socket while a domestic charge is active immediately cuts power to the domestic socket and switches priority to the EV.
- **`2PCNOK`**: Software inhibition of the domestic socket.
- **Current Telemetry**: Current draw on the domestic socket is reported via standard `CCI:` frames through the shared Phase 1 current transformer.

---

### 5.6 Physical DIP Switches & Supervisory Override (`RaspberryPiModeOK`)

- **Switches 1 & 2 (Remote Controls / Conditioning)**:
  - Switch 1: Contact 2 conditioning (`FM2:2`, not overridable by `DOK`).
  - Switch 2: Contact 1 conditioning (`FM2:1`, overridable by `DOK`). Setting Switch 2 to `ON` in standalone mode enforces `FM2:1` every second.
- **Switches 3 & 4 (Maximum Station Current)**: Encodes boot-time hardware maximum limits (`16A`, `20A`, `25A`, `32A`).
- **`RaspberryPiModeOK` Reporting & Override**:
  - Reports `DIP1`..`DIP4` in reverse order of the physical board switches (Switch 4 = `DIP1`, Switch 3 = `DIP2`, Switch 2 = `DIP3`, Switch 1 = `DIP4`).
  - Disables runtime polling of Switches 1 & 2, but preserves the boot-time current limit (`CCS`) from Switches 3 & 4 unless overridden via `CCS:<val>`.

---

### 5.7 Soft Pause & Resume: The `SB` Button Mechanism (`pause` / `resume`)

For transient pauses during an ongoing charging session (e.g. temporary solar dips or EVCC pause cycles), Green'Up Link simulates the station's physical Start/Stop push button:
- **`action: "pause"`**: Sends `SBNOK` (simulates pressing the Stop button). Sets `SB` to `0`. The station safely ramps down current, passes through transient `State:W`, and stabilizes in **`State:M`** (`SB:0`).
- **`action: "resume"`**: Sends `SBOK` (simulates pressing the Start button). Sets `SB` to `1`. The station returns to `SB:1` and resumes charging (`State:C` $\rightarrow$ `State:D`/`State:E`).

> [!WARNING]
> **Non-Blocking Behavior of `SB` upon Cable Replug:**  
> Unlike `FM2:1`, the `SBNOK` pause state is **not persistent across physical connection events**.  
> If the station is paused via `SBNOK` (`SB:0` in `State:M`), and the user **unplugs and replugs the charging cable into the vehicle**, the ATmega automatically resets `SB` back to `1` and clears the pause!  
> For this reason, `pause`/`resume` should be reserved for temporary in-session pauses, while `enable`/`disable` is the authoritative session lock.

---

### 5.8 Current Setpoints: Protocol Formatting, Safety Clamping & `CCEl` vs `CC`

When modulating charging current limits via `POST /api/current/:amps` or MQTT:

#### 1. Mandatory Two-Digit Formatting (`{:02}`)
The ATmega serial parser strictly expects a 2-digit integer for all current setpoints (e.g. `CC:09`, `CCEl:16`).  
Sending a single digit (such as `CC:9`) causes the ATmega parser to misread the parameter, which evaluates internally to `0A` (`CC:00`), inducing immediate regulation failures.

#### 2. The 6A Boundary & The 7A Operational Safety Floor
The IEC 61851-1 specification defines 6A as the normative minimum pilot current (10% PWM duty cycle).
- **Sub-6A Firmware Crash (`< 6A`):** If a setpoint strictly below 6A is received by the ATmega (such as `CC:05`, `CCEl:05`, or `CC:00`), the station fails to regulate cleanly, panics after approximately 8 seconds with error **`E:0010`**, and **forces a full hardware reboot (`State:X`)**!
- **Real-World Calibration at 6A:** While `CC:06` or `CCEl:06` is accepted by the firmware without crashing, field measurements demonstrate that the actual physical current delivered by the station at a 6A setpoint is very close to 7A. This slight calibration offset brings the station dangerously close to its internal overcurrent safety threshold.
- **Safety Clamp at 7A:** To prevent tripping overcurrent protections and ensure rock-solid stability, Green'Up Link intentionally clamps all dynamic current setpoints to a strict minimum of **7A**.

#### 3. Why Green'Up Link Modulates `CCEl` Rather Than Direct `CC`
The Legrand ATmega power board implements multiple concurrent current limiting registers:
- `CC`: The effective active current limit currently applied to the Control Pilot PWM. **Warning**: This register is volatile in RAM and is frequently overwritten by internal station routines back to the nominal maximum station current (`CCS`) during re-arm sequences.
- `CCEl`: The non-volatile Eco-Logic / Eliot supervisory limit written directly to EEPROM. It is never overwritten by background routines.
- `CCTIC`: The autonomous grid load-shedding limit dynamically calculated from the TIC interface.
- `CCCa`: The physical cable rating detected via the Proximity Pilot (PP) resistor (e.g. 13A, 20A, 32A).
- Hardware Jumpers / DIP switches: The physical installation maximum (e.g. 16A or 32A).

The ATmega determines the effective pilot current dynamically by computing the **minimum** across all active constraints:
$$\text{Effective Limit} = \min(\text{Hardware Dipswitch}, \text{Cable PP (CCCa)}, \text{CCEl}, \text{CCTIC})$$

**Why using `CCEl` is safer and more stable:**
- `CCEl` writes directly into EEPROM and persists without background resets.
- In nominal operation, setting `CCEl` regulates the charging speed just like `CC`.
- However, modulating `CCEl` preserves all underlying hardware and safety limits: if an undersized cable is plugged in, or if the physical TIC triggers emergency load-shedding (`CCTIC`), the ATmega automatically caps the charge to that lower value. Writing `CCEl` lets the ATmega exercise its full safety checks.
- **State A Synchronization:** When the station is idle in `State:A` (no vehicle plugged in), Green'Up Link synchronizes both `CCEl` and `CC` so that the station's baseline default matches the requested limit before a vehicle is attached. Once a vehicle is connected and a session is active, Green'Up Link exclusively modulates `CCEl`.

---

## 6. Error Codes (`E:XXXX`)

When the station enters a fault state (`State:R`, `State:S`, `State:T`, `State:U`, `State:V`), it emits an error code queried via `E?`:

| Code | OCPP Fault Category | Physical Explanation |
| :--- | :--- | :--- |
| **`E:0000`** | `NoError` | No error detected. |
| **`E:0001`** | `ConnectorLockFailure` | Type 2 socket locking failure. |
| **`E:0002`** | `ConnectorLockFailure` | Type 2 socket unlocking failure. |
| **`E:0003`** | `OtherError` | Control Pilot short-circuit detected on socket, cable, or vehicle side. |
| **`E:0004`** | `OtherError` | Control Pilot short-circuit detected on power board side. |
| **`E:0005`** | `PowerSwitchFailure` | Domestic socket contactor opening fault. |
| **`E:0006`** | `PowerSwitchFailure` | Domestic socket contactor closing fault. |
| **`E:0007`** | `PowerSwitchFailure` | Type 2 socket contactor opening fault. |
| **`E:0008`** | `PowerSwitchFailure` | Type 2 socket contactor closing fault. |
| **`E:0009`** | `OtherError` | Diode not detected on vehicle side. |
| **`E:0010`** | `OverCurrentFailure` | Current overload on Type 2 socket *(also triggered if setpoint < 6A)*. |
| **`E:0011`** | `OverCurrentFailure` | Current overload on Domestic Schuko socket. |
| **`E:0012`** | `UnderVoltage` | Mains power outage / blackout (triggers `State:V`). |
| **`E:0013`** | `OtherError` | Internal serial communication fault. |
| **`E:0015`** | `OtherError` | 6mA DC residual current leakage fault detected. |

---

## 7. Functioning Modes (`FM`)

> [!IMPORTANT]
> **Dynamic Calculation of `FM?`:**  
> The query `FM?` does not retrieve a static mode number. Instead, the ATmega computes this value dynamically from internal state and configuration flags:
> - **`FM:1`**: Direct Mode (Nominal) — returned when no derogation, external contact conditioning, planning, or remote supervision is asserted.
> - **`FM:2`**: Contact Conditioning Active — returned as soon as dry contact conditioning is active (`FM2:1`, `FM2:2`, `FM2:3`).
> - **`FM:4`**: Weekly Calendar / Planning mode (Bluetooth schedule).
> - **`FM:5`**: Modbus RTU Slave mode (RS485 `Serial3`).
> - **`FM:6`**: Remote OCPP Cloud supervisor.
> - **`FM:3` does not exist**: Sending `FM:3` hits default handling in firmware and automatically forces the station back to `FM:1`.
> 
> **Validated Mode for Green'Up Link: `FM:1` (Direct Charge)**  
> **Only `FM:1` (Direct Charge / Permanent) has been regularly used, tested, and validated with Green'Up Link.**  
> While modes 2, 4, and 5 are theoretically compatible with specific hardware setups, they have not been thoroughly tested. Mode 6 (OCPP Cloud) introduces significant timing alterations (a mandatory 30-second RFID authorization delay, altered unlock commands) and is likely incompatible with local supervision.  
> **To operate Green'Up Link reliably, ensure your station is configured in mode `FM:1`.**

The station's active functioning mode can be queried with `FM?` and set via `FM:X`:

| Mode | Official Designation | Explanation & Green'Up Link Compatibility |
| :--- | :--- | :--- |
| **`FM:1`** | **Direct Charge (Permanent)** | **Validated & Recommended.** Returned dynamically when no conditioning flags are set. The station delegates session control to the supervisory port. Green'Up Link manages authorization cleanly via `D` + `FM2`. |
| **`FM:2`** | **Remote Controls (Auto-Hours / Forcing)** | Returned as soon as Contact 1 or Contact 2 conditioning is engaged (`FM2:1`, `FM2:2`, `FM2:3`). Relies on external dry contact or physical TIC for Peak/Off-Peak switching. |
| **`FM:3`** | **Does Not Exist** | Not implemented in the ATmega firmware (aliased to `FM:1`). |
| **`FM:4`** | **Programming (Planning)** | Weekly calendar scheduling mode (standalone only; only mode where `Calendar?` is evaluated). |
| **`FM:5`** | **Modbus RTU Slave** | Station controlled via RS485 Modbus RTU (`Serial3`). |
| **`FM:6`** | **OCPP Cloud** | Cloud supervision mode. Disables TIC detection, enforces a 30s RFID authorization wait, and modifies unlock logic. **Incompatible with Green'Up Link.** |
