# Legrand Charging Station State Machine (Green'Up Premium)

This document details the operation of the proprietary state machine in the Legrand ATmega firmware.
The states returned by the `State:` serial command are not simple IEC 61851 electrical states, but a true behavioral state machine.

## 1. Full list of states (State:X)

This semantics was established with certainty by reverse-engineering the power board's 18.04 firmware and cross-referencing it with the original Java implementation.

| State | Hardware interpretation | OCPP mapping (according to ATmega) |
|---|---|---|
| **A** | Idle, no vehicle connected | Available |
| **B** | **T2S** socket occupied, waiting for authorization | SuspendedEVSE triggers StartTransaction |
| **C** | T2S → charge suspended by vehicle | SuspendedEV |
| **D** | T2S → **charging in progress** | Charging |
| **E** | T2S → **charging in progress** (variant) | Charging |
| **F** | **Domestic 2P+T** socket occupied, waiting | SuspendedEVSE triggers StartTransaction |
| **G** | Domestic → **charging in progress** | Charging |
| **H** | Domestic → **charging in progress** (variant) | Charging |
| **I** | T2S → suspended by vehicle (variant) | SuspendedEV |
| **J** | T2S → suspended by station | SuspendedEVSE |
| **K** | Suspended by station | SuspendedEVSE |
| **L** | **Vehicle unplugged** / end of session | stop, reason EVDisconnected |
| **M** | **Local stop** (button) | stop, reason Local |
| **N** | Suspended by station (T2S) | SuspendedEVSE |
| **O** | Suspended by station (domestic) | SuspendedEVSE |
| **P** | Local stop in progress | stop, reason Local |
| **R** | **Fault** (lock, overload, contactor) | Faulted, Other |
| **S** | **Fault** (contactor, CP) | Faulted, Other |
| **T** | **Fault** (CP short-circuit) | Faulted, Other |
| **U** | **Power loss** | PowerLoss |
| **V** | **Power loss** (variant) | PowerLoss |
| **W** | Transient state, emitted but never stable | ignored |
| **X** | Reboot | ignored |
| **Y** | Deep **sleep** (command Slp:1) | Unavailable |
| **Z** | Initialization / production mode | Unavailable |

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

## 3. Custom IEC states mapping (Level 2)

To expose a clean and stable state machine to a supervisor (like EVCC or EVerest), we use an abstraction layer (Level 2) that translates native Legrand states into standardized IEC 61851 states: **Disconnected_A**, **Connected_B**, **Charging_C**, **Error_E** (Minor/recoverable fault), and **Faulted_F** (Fatal hardware fault).

The logic chosen for the `greenup-everest` driver is as follows:

**Special Case (Station software locked):**
If `T2C:0`: The station is disabled and its state machine is blind (`State` remains stuck in `A`). The IEC state is deduced exclusively from the Control Pilot voltage (`CP?`):
*   `CP:12` ➡️ **Disconnected_A**
*   `CP:9` ➡️ **Connected_B**
*   `CP:6` ➡️ **Charging_C**
*(Design note: The driver will have to set `T2C:1` when it wants to reauthorize charging).*

**Nominal Case (T2C:1):**
The Legrand state machine is consistent and can be translated directly:
*   `State:A` (Idle) ➡️ **Disconnected_A**
*   `State:L` (Unplugging) ➡️ **Disconnected_A** *(automatically transitions to A)*
*   `State:B` (Connected, waiting for station/TIC) ➡️ **Connected_B**
*   `State:C` (Waiting for EV / Ready) ➡️ **Connected_B**
*   `State:I` (Interrupted by EV) ➡️ **Connected_B** *(warning, automatically loops back to A->B->C, but logically remains B)*
*   `State:W` (Stopping in progress) ➡️ **Connected_B**
*   `State:M` (Manual stop / via `SBNOK`) ➡️ **Connected_B** *(stable state as long as EV is not unplugged/replugged)*
*   `State:D` / `State:E` (Charging) ➡️ **Charging_C**
*   `State:R` / `State:X` (Fault / Reboot) ➡️ **Error_E**
*   `State:V` (Fatal power loss) ➡️ **Faulted_F** *(occurs with E:0012 right before shutdown)*

## 4. Inconsistencies found in logs (Quirks)

### The A-B-C automatic sequence in FM:1
In `FM:1` (Direct Charge) mode, the A-B-C state sequence is automatic upon connecting the vehicle. Without external intervention, it goes straight to `State:C` and then resumes charging (`State:E`). 
Only 2 elements can prevent this behavior:
1. `T2C:0` (after a `T2CNOK` command), which blocks the station in `State:A`.
2. The presence of TIC (Tele-Information Client), which blocks the station in `State:B` during peak hours (HP).

This `FM:1` mode deprives us of authorization control: the station decides to charge on its own. To implement Smart Charging where the driver decides *when* charging should start, `FM:1` is not well suited out-of-the-box unless we actively use other commands to pause it (like `SBNOK`).

### State:A forced despite plugged cable after local stop
If we force charging to stop from the station using the `T2CNOK` command, the station performs its closing sequence (passing through W then M) and then falls back to `State:A` (Available). **However, the cable on the vehicle side is still physically plugged in.** Logically, the station should return to `State:B` (Socket occupied, waiting for authorization), but it declares itself completely free.
**Consequence (Software lock of the socket):** The `T2CNOK` command (which sets the internal `T2C` variable to `0`) not only forces the station into `State:A`, but **locks the Type 2 socket state machine in this state**. As long as `T2C:0`, absolutely all physical events on this socket (insertion, removal of cable, button press) are ignored by the firmware. The API is therefore blind to the physical state of the cable.
**Workaround (Analog Polling):** Even if asynchronous events are locked, the API can directly poll the Analog-to-Digital Converter (ADC) with the `CP?` command. This returns the raw voltage on the Control Pilot pin: `12` (Unplugged), `9` (Plugged/State B), `6` (Charging/State C). Polling `CP?` restores visibility!

### The ghost Eco-Start (Off-Peak) mode and its workaround
Normally, the `FM2:0` command is supposed to disable the eco-start function (so charging should start instantly). However, logs show that the TIC Peak/Off-Peak (HP/HC) control remains active despite `FM2:0`. 
This behavior unexpectedly blocks the state machine in `State:B` during Peak Hours (HP).

**Workaround:** A way to temporarily override this (for a single charging session) is to explicitly resend the `FM:1` command while the station is in `State:B`. 
**WARNING:** Doing this completely cuts off the Peak/Off-Peak control, but it also disables the TIC load balancing for the entire duration of that charging session.

### The CC: parser bug and the sub-6A crash
Analysis of current setpoints revealed two consecutive flaws:
1. **The `CC:` parser is buggy**: It strictly expects 2 digits. If we send `CC:9` instead of `CC:09`, the station misinterprets it and applies a `0A` setpoint (it replies `CC:00`).
2. **Crash under 6A**: The IEC 61851 standard requires a minimum current of 6A. If the station receives a setpoint strictly below 6A (like `CC:05`, or `0A` due to the previous bug), it does not stop the charge cleanly. The current drops to the hardware minimum (~6.8A), then after 8 seconds the station panics. It emits the fault `E:0010` (`State:R`), closes the session, and **reboots completely** (`State:X`)!
Therefore, the driver must strictly format setpoints to 2 digits and impose a strict low software limit of **7A**.

### Interruption via Unlock
The `Unlock` command cleanly forces a charge in progress to stop. It triggers the end sequence (`State:W` -> Send summary -> `State:A`).
However, since the cable is physically still plugged in, the station immediately detects the contactor (`SBF:1`) and goes back to `State:B`. Due to the automatic A-B-C sequence in `FM:1` mode (explained above), it will then immediately restart the charge, creating an infinite loop.

### The major discovery of Start/Stop control via SBOK / SBNOK
Smart Charging requires being able to pause and resume a charge without locking the system. Using `Unlock` loops infinitely, and `T2CNOK` blinds the state machine.
**The ultimate solution lies in the `SBOK` and `SBNOK` commands.**
These commands simulate a software press on the physical STOP/START button on the front panel.
*   Sending **`SBNOK`**: The station thinks the STOP button was pressed. It immediately goes to `State:W` (Stopping), sends the session ticket, then safely stabilizes in **`State:M`** (Manual stop). It doesn't loop, it waits.
*   Sending **`SBOK`**: The station thinks START was pressed (or the cable was plugged). It goes back to `State:A`, detects the socket (`SBF:1`), emits `Start`, goes to `State:B`, then starts charging (`State:C`).

This is the **perfect** mechanic to control charge and load balancing sessions for EVerest / Home Assistant integration, without suffering the side effects of other interruption commands!

## 5. Error Codes (E:XXXX)

When the station enters a fault state (`State:R`, `State:S`, `State:T`, `State:U`, `State:V`), it usually emits an error code that can be queried with `E?`.
Here is the official interpretation table of error codes extracted from the original Java code (OCPP mapping):

| Code | OCPP Fault Category | Physical Explanation |
| :--- | :--- | :--- |
| **`E:0000`** | `NoError` | No error detected. |
| **`E:0001`** | `ConnectorLockFailure` | T2S socket locking error. |
| **`E:0002`** | `ConnectorLockFailure` | T2S socket unlocking error. |
| **`E:0003`** | `OtherError` | Control Pilot short-circuit detected on socket, cable or vehicle side. |
| **`E:0004`** | `OtherError` | Control Pilot short-circuit detected on board side. |
| **`E:0005`** | `PowerSwitchFailure` | Domestic socket contactor opening error. |
| **`E:0006`** | `PowerSwitchFailure` | Domestic socket contactor closing error. |
| **`E:0007`** | `PowerSwitchFailure` | T2S socket contactor opening error. |
| **`E:0008`** | `PowerSwitchFailure` | T2S socket contactor closing error. |
| **`E:0009`** | `OtherError` | Diode not detected on vehicle side. |
| **`E:0010`** | `OverCurrentFailure` | Current overload on T2S socket. *(Also triggered in case of PWM crash / setpoint < 6A!)* |
| **`E:0011`** | `OverCurrentFailure` | Current overload on Domestic socket. |
| **`E:0012`** | `UnderVoltage` | Power outage / Electrical power failure (triggers `State:V`). |
| **`E:0013`** | `OtherError` | Internal USB communication error. |
| **`E:0015`** | `OtherError` | 6mA DC leakage fault detected. |

## 6. Functioning Modes (FM)

The ATmega has several internal functioning modes that alter its state machine logic. The main mode can be modified by sending `FM:X` (where X is the mode number) and verified with `FM?`.

| Mode | Official designation | Explanation of the mode |
| :--- | :--- | :--- |
| `FM:1` | **Direct Charge (Permanent)** | The station charges as soon as a vehicle is plugged in, without any condition. This is the "dumb executor" mode we force by default in Green'Up Link. |
| `FM:2` | **Remote controls (Auto-hours)** | "Peak / Off-Peak" mode, relying on the station's contactor input (Dry contact) or TIC to start/stop the charge. |
| `FM:3` | **Smart meter (TIC Linky)** | Intelligent control based on the Linky meter's tele-information. **Note:** This mode is actually **not implemented in the ATmega code** (firmware 18.04), which explains why Legrand hid it (commented it out) in the original Web interface code. |
| `FM:4` | **Programming (Planning)** | Internal time programming mode. The ATmega relies on calendar files to trigger the charge. |
| `FM:5` | **Modbus (DLM)** | In this mode, the station is controlled via the RS485 bus (Modbus protocol) by an external energy manager (Load Management). |
| `FM:6` | **OCPP** | Cloud supervision mode. The station awaits its orders from the central OCPP server. **Warning:** this mode alters the ATmega's internal behavior (disables TIC auto-detection, imposes a 30s blocking authorization wait after presenting a badge, modifies the `Unlock` command logic, and forces RFID flags to 1 at startup). |
