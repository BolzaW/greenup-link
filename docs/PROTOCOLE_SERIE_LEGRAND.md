# Serial Protocol - Legrand GreenUp Power Board

This document lists the ASCII commands used to communicate with the power board.
These commands were deduced from reverse-engineering activities and decompilation of the original software (.jar) and firmware (.hex).
All these commands must be sent to the power board followed by a "Carriage Return" character, i.e. `\r` (ASCII code 0x0D).

## 🔌 1. Initialization & System
These commands are used at startup to establish dialogue.

| Command | Supposed explanation |
| :--- | :--- |
| `RaspberryPiModeOK` | Magic frame that tells the power board that the Pi has taken control. |
| `SoftwareVersion?` | Requests the power board's firmware version. |
| `HardwareVersion?` | Requests the board's hardware version. |
| `SerialNumber?` | Requests the station's serial number. |
| `Reference?` | Requests the Legrand product reference. |
| `WeekYearProduction?`| Requests the manufacturing date (Week/Year). |
| `Side?` | Requests the active side (Side 1 or 2). Often useful on dual-socket stations. |
| `Reset` | Reboots the power board. |
| `Test` | Puts the board into a factory or lab "Test" mode. |
| `ping` | Basic ping (probably to check if the board is still alive). Replies `pong`. |

## 📊 2. Status & Telemetry
These commands are used to query the station about its current state.

| Command | Supposed explanation |
| :--- | :--- |
| `State?` | Requests the transactional/behavioral state of the charge (A, B, C, D, E, I, W, M). Warning: this is NOT the raw IEC 61851 physical state. See docs/MACHINE_A_ETATS.md for full details. |
| `E?` | Requests current errors (returns `E:0000` if all is well). Here is the official interpretation table of error codes extracted from the Java code (OCPP mapping):<br>• **`E:0000`** : `NoError`<br>• **`E:0001`** : `ConnectorLockFailure` (T2S socket locking error)<br>• **`E:0002`** : `ConnectorLockFailure` (T2S socket unlocking error)<br>• **`E:0003`** : `OtherError` (Control Pilot short-circuit detected on socket, cable or vehicle side)<br>• **`E:0004`** : `OtherError` (Control Pilot short-circuit detected on board side)<br>• **`E:0005`** : `PowerSwitchFailure` (Domestic socket contactor opening error)<br>• **`E:0006`** : `PowerSwitchFailure` (Domestic socket contactor closing error)<br>• **`E:0007`** : `PowerSwitchFailure` (T2S socket contactor opening error)<br>• **`E:0008`** : `PowerSwitchFailure` (T2S socket contactor closing error)<br>• **`E:0009`** : `OtherError` (Diode not detected on vehicle side)<br>• **`E:0010`** : `OverCurrentFailure` (Current overload on T2S socket. *Also triggered in case of PWM crash / setpoint < 6A!*)<br>• **`E:0011`** : `OverCurrentFailure` (Current overload on Domestic socket)<br>• **`E:0012`** : `UnderVoltage` (Power outage / Electrical power failure)<br>• **`E:0013`** : `OtherError` (Internal USB communication error)<br>• **`E:0015`** : `OtherError` (6mA DC leakage fault detected) |
| `CC?` | Requests the configured current limit (e.g., returns `CC:16`). |
| `CCTIC?` | Requests the current limit value deduced by the TIC. Returns `32` even if the TIC is disconnected. |
| `TICTM:1` / `TICTM:0` | Enables (`1`) or disables (`0`) the TIC Test mode. In test mode, the station returns `TICTestC:Init` then periodically `TICTestB:X` where `X` is the baud rate (`0` if absent, `1200` if Historic, `9600` if Standard). Then, it continuously broadcasts `TICTestC:XX` where `XX` is the dynamic charging limit (CCTIC) calculated by the board for load shedding. |

## ⚡ 3. Current / Power Measurements and Setpoints
These variables help exactly understand what limits are imposed and what is consumed.

**Queryable variables (with `?`) or received as echo:**
*   **`CCCa`** (*max current cable*): Hardware capacity of the cable (read via the PP resistor). Ex: `CCCa:32` for 32A.
*   **`CCS`** (*max current station*): Hardware capacity of the station (set via internal DIP switches). Ex: `CCS:32`.
*   **`CCEl`** (*max current eliot*): Limit imposed by the Cloud (Eliot / Legrand App).
*   **`CC`** (*charging current*): The final setpoint retained and imposed by the ATmega (often the minimum of the previous limits).
*   **`CP`** (*control pilot*): Raw voltage measured on the Control Pilot pin. **Vital:** Querying `CP?` reveals the actual physical connection state (`12`=unplugged, `9`=plugged, `6`=charging) and bypasses the state machine software lock caused by `T2CNOK`.

**Spontaneous variables (emitted by the station):**
*   **`CCI:X.XX`** (*current instantaneous*): Actual instantaneous current drawn by the vehicle (in Amperes). Drops at the end of the charge.
*   **`CPh:Mono` / `CPh:Tri`** (*charge phases*): Automatic detection of the number of phases used by the vehicle. Emitted just before the charge ramps up.

## 🎮 4. Charge Control (The most important!)
These commands directly control power delivery.

| Command | Supposed explanation |
| :--- | :--- |
| `CC:16` | **Charge Current** : Sets the power limit of the main socket to X Amps (e.g. 16, 32). |
| `CCEl:16` | **Charge Current Eliot** : Sets the Cloud power limit. Safer than `CC:` because the ATmega guards its minimum hardware values. |
| `CCS:16` | **Charge Current Schuko** : Sets the power limit for the domestic socket (Schuko). |
| `T2COK` / `T2CNOK` | Authorizes (`OK`) or Blocks (`NOK`) charging on the **Type 2** (T2) socket. Warning: `T2CNOK` blinds the state machine. |
| `2PCOK` / `2PCNOK` | Authorizes (`OK`) or Blocks (`NOK`) charging on the **Domestic** (2 Pins) socket. |
| `T2FOK` / `T2FNOK` | **Forces** charging on the Type 2 socket (bypasses safety or schedule?). |
| `2PFOK` / `2PFNOK` | **Forces** charging on the domestic socket. |
| `SOK` / `SNOK` | **Sleep**: `SOK` forces the station into deep sleep mode (`State:Y`, LEDs off with slow flash, answers `Slp:1`). This mode can only be reached from `State:A`. `SNOK` wakes the station up (`Slp:0`, `State:A`). |
| `SBOK` / `SBNOK` | **Absolute software control commands (Pause/Resume)**. Simulates pressing the physical START/STOP button on the front panel. `SBNOK` cleanly stops the charge and stabilizes the station in `State:M`. `SBOK` wakes the station up and restarts the cycle (`State:B` -> `State:C`). Not to be confused with the `SBF` read event. |
| `Unlock` | Orders the **physical unlocking** of the cable (if the station has a Type 2 socket lock). |

## 💳 5. OCPP / RFID Management
These commands manage interactions with the user badge or software supervision.

| Command | Supposed explanation |
| :--- | :--- |
| `OCPPPS?` | OCPP Status : Requests the station's OCPP status. It replies with an OCPP state (e.g. `OCPPStatus:Available` or `OCPPStatus:SuspendedEVSE`). |
| `OCPPPACOK` / `OCPPPACNOK` | **P**lug **A**nd **C**harge : Indicates to the board whether the badge-less "Plug and Charge" function is enabled. |
| `RFIDId:XXXXX` | Transmits the RFID badge ID (read by the Pi) to the power board. |
| `RFIDA:XXXXX` | Sends the RFID authorization status. |
| `OCPPCTO:XXX` | Connection Time Out : Sets the connection timeout. |

*Note for tests: In the `cli_greenup-link.ps1` terminal, you do not need to type `\r`, the program adds it automatically upon pressing Enter.*

## 📻 6. Bluetooth (BLE) Module Management
The station has an integrated Bluetooth module (used by the smartphone app). It can be enabled or disabled to avoid command conflicts with the Raspberry Pi.

| Command (TX) | Reply (RX) | Explanation |
| :--- | :--- | :--- |
| `BT?` | `BT:1` or `BT:0` | Requests the current state of the Bluetooth module (1 = On, 0 = Off). |
| `BTOK` | `BT:1` | Orders the Bluetooth module to turn on. |
| `BTNOK`| `BT:0` | Orders the Bluetooth module to turn off. |

## 🧾 7. End of Charge Reports (Session Tickets)
At the end of a charging session (when the station passes through states W then M before returning to A), the power board automatically emits a summary frame containing the session data.

**Example RX frame:**
`\WT:0:0:5:CT:0:6:58:EVplug:4485.20:0.00:\`

**Decryption (deduced from the original Java source code):**
This frame is separated by colons (`:`).
*   `WT` : **Waiting Time**.
*   `0:0:5` : Waiting duration in Hours:Minutes:Seconds (here 5 seconds).
*   `CT` : **Charging Time**.
*   `0:6:58` : Effective charging duration in Hours:Minutes:Seconds.
*   `EVplug` : Identifier of the socket used (`EVplug` = Type 2, `DOMplug` = Domestic E/F socket).
*   `4485.20` : **Average Power (in Watts)** delivered during Peak Hours (HP).
*   `0.00` : **Average Power (in Watts)** delivered during Off-Peak Hours (HC).

*Note on energy: The board does not directly report Wh, but average power. To obtain the total session energy in Wh, the original Java daemon applied the formula: `Energy (Wh) = Average Power (W) * (Charging Time (min) / 60)`.*

## ⚙️ 8. Functioning Modes (FM)
The ATmega has several internal functioning modes. The main mode can be modified by sending `FM:X` (where X is the mode number) and verified with `FM?`.

| Mode | Official designation | Explanation of the mode |
| :--- | :--- | :--- |
| `FM:1` | **Direct Charge (Permanent)** | The station charges as soon as a vehicle is plugged in, without any condition. This is the "dumb executor" mode we force by default in Green'Up Link. |
| `FM:2` | **Remote controls (Auto-hours)** | "Peak / Off-Peak" mode, relying on the station's contactor input (Dry contact) or TIC to start/stop the charge. |
| `FM:3` | **Smart meter (TIC Linky)** | Intelligent control based on the Linky meter's tele-information. **Note:** This mode is actually **not implemented in the ATmega code** (firmware 18.04), which explains why Legrand hid it (commented it out) in the original Web interface code. |
| `FM:4` | **Programming (Planning)** | Internal time programming mode. The ATmega relies on calendar files to trigger the charge. |
| `FM:5` | **Modbus (DLM)** | In this mode, the station is controlled via the RS485 bus (Modbus protocol) by an external energy manager (Load Management). |
| `FM:6` | **OCPP** | Cloud supervision mode. The station awaits its orders from the central OCPP server. **Warning:** this mode alters the ATmega's internal behavior (disables TIC auto-detection, imposes a 30s blocking authorization wait after presenting a badge, modifies the `Unlock` command logic, and forces RFID flags to 1 at startup). |

### External Signal (FM2)
In addition to the main mode, the station manages a secondary parameter (`FM2`) which corresponds to the external signal (Dry Contact or TIC Peak/Off-Peak):
*   `FM2?` : Queries the current state of the external signal.
*   `FM2:0` : Simulates an Open contact (Peak Hours - HP). If the station is in `FM:2` without a TIC, this suspends the charge (`State:B`).
*   `FM2:1` : Simulates a Closed contact (Off-Peak Hours - HC). If the station is in `FM:2` without a TIC, this authorizes the charge (`State:C`). 

*(Technical note: The Java code logs `FM2` as `external signal`. Sending `FM2:0` was previously mistakenly thought to disable the Eco-Start feature, but it actually injects a software state for Peak Hours, which can unexpectedly freeze the state machine in `State:B` if not careful).*
