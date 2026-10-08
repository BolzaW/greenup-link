# Legrand Serial Protocol (GreenUp Power Board)

This document lists the ASCII commands used to communicate with the power board.
These commands were deduced from reverse-engineering activities and decompilation of the original software (.jar) and firmware (.hex).
All these commands must be sent to the power board followed by a "Carriage Return" character, i.e., `\r` (ASCII code 0x0D).

*Note: In the `cli_greenup-link.ps1` terminal, you do not need to type `\r`; the program adds it automatically.*
*Note 2: If a command is not recognized or is malformed, the ATmega will respond with `Default:<Command>`.*

---

## 1. System & Initialization
Commands used at startup to establish dialogue and identify the board.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `RaspberryPiModeOK` | `Side:` | Magic frame that tells the power board that the Pi has taken control. Acknowledged by the board dumping its active side. |
| `SoftwareVersion?` | `SoftwareVersion:` | Requests the power board's firmware version. |
| `HardwareVersion?` | `HardwareVersion:` | Requests the board's hardware version. |
| `SerialNumber?` | `SerialNumber:` | Requests the station's serial number. |
| `Reference?` | `Reference:` | Requests the Legrand product reference. |
| `WeekYearProduction?`| `WeekYearProduction:`| Requests the manufacturing date (Week/Year). |
| `Side?` | `Side:` | Requests the active side (Side 1 or 2). Often useful on dual-socket stations. |
| `Reset` | `State:` | Reboots the power board. Acknowledged by the board's first state upon rebooting. |
| `Z` | None | **⚠️ CRITICAL WARNING:** Triggers a software reset and drops the ATmega into **Bootloader mode** for firmware flashing. Do NOT use unless you intend to run `avrdude` immediately after. |
| `ping` | `pong` | Basic ping to check if the board's serial interface is responsive. |

---

## 2. Charge Telemetry & Status
Commands to query the real-time behavioral state and errors of the station.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `State?` | `State:` | Requests the transactional/behavioral state of the charge (A, B, C, D, E, I, W, M). *See `STATE_MACHINE.md` for full details.* |
| `E?` | `E:` | Requests current errors. `E:0000` means no error. Errors `E:0001` to `E:0015` indicate hardware faults. *See Chapter 5 of STATE_MACHINE.md for the full error codes table.* |
| `CP?` | `CP:` | Raw voltage measured on the Control Pilot pin. Reveals the actual physical connection state (`12`=unplugged, `9`=plugged, `6`=charging). Vital to bypass the software state machine lock. |
| `SB?` | `SB:` | Reads the current state of the front panel START/STOP button. |
| `T2C?` | `T2C:` | Reads the current authorization state of the Type 2 socket. |

---

## 3. Current & Power Configuration
Commands to read or set charging current limits.
*Note: The final charging current (`CC`) is determined by the ATmega as the minimum of the hardware capability (`CCCa`, `CCS`) and software limits (`CCEl`, `CCTIC`).*

**⚠️ Important Constraints:**
- **Current Limits:** Tested and supported values range strictly from **`07`A to `32`A**. The board cannot regulate PWM below `6.8A` and will crash into an `E:0010` (Overcurrent) fault if configured to `05`A or lower.
- **Over-provisioning:** It is software-wise possible to request more than 32A (e.g., `CC:33`), but this has **never been tested in real conditions** and must be strictly avoided to prevent hardware damage.
- **Formatting:** All set commands require exactly **two digits** for the value (e.g., `08`, `16`, `32`).

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `CC?` | `CC:` | Requests the final calculated charging current limit imposed by the ATmega (e.g., `CC:16`). |
| `CC:XX` | `CC:` | Overrides the `CC` variable directly (format `XX` mandatory). **Warning**: Potentially more dangerous than using `CCEl:XX` because there are no internal safeguards (the ATmega does not guard its minimum hardware values). |
| `CCCa?` | `CCCa:` | Requests the hardware current limit of the plugged-in Type 2 cable (read from the PP resistor). |
| `CCEl?` | `CCEl:` | Requests the software current limit imposed by the Cloud (Eliot / Legrand App). |
| `CCEl:XX` | `CCEl:` | Sets the Cloud power limit (e.g., `CCEl:16`, format `XX` mandatory). Safer than overriding `CC` directly. |
| `CCS?` | `CCS:` | Requests the maximum current capacity configured for the Schuko socket. |
| `CCS:XX` | `CCS:` | Sets the power limit for the domestic socket (Schuko) (format `XX` mandatory). |
| `CCTIC?` | `CCTIC:` | Requests the dynamic current limit deduced by the TIC. Returns `32` if the TIC is disconnected. |
| `TICTM:1` / `TICTM:0` | `TICTM:1` / `TICTM:0` | Enables (`1`) or disables (`0`) the TIC Test mode. When enabled, the station broadcasts TIC baud rates and calculated limits (`CCTIC`). |

---

## 4. Hardware & Socket Control
Low-level commands to interact with physical buttons, enable, disable, or force the physical sockets.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `T2COK` / `T2CNOK` | `T2C:1` / `T2C:0` | Authorizes (`OK`) or Blocks (`NOK`) charging on the **Type 2** socket. *Warning: `T2CNOK` blinds the state machine.* |
| `2PCOK` / `2PCNOK` | `2PC:1` / `2PC:0` | Authorizes (`OK`) or Blocks (`NOK`) charging on the **Domestic** (Schuko / 2 Pins) socket. |
| `T2FOK` / `T2FNOK` | `T2F:1` / `T2F:0` | **TBD**: **Forces** charging on the Type 2 socket (bypasses safety or schedule?). Not extensively tested. |
| `2PFOK` / `2PFNOK` | `2PF:1` / `2PF:0` | **TBD**: **Forces** charging on the domestic socket. Not extensively tested. |
| `SBOK` / `SBNOK` | `SB:` | **Pause/Resume**: Simulates pressing the physical START/STOP button on the front panel. `SBNOK` cleanly stops the charge (`State:M`). `SBOK` wakes the station and restarts the cycle. *Note: `SBNOK` can be overridden by a spontaneous `SBF:1` emitted by the board when a vehicle is plugged in.* |
| `Unlock` | None | Orders the **physical unlocking** of the cable (if the station has a Type 2 socket lock mechanism). |

---

## 5. Smart Charging & Session Control
Commands to manage functioning modes, schedules, and active sessions.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `FM?` | `FM:` | Requests the main Functioning Mode (1=Direct Charge, 2=Auto-hours/TIC, 4=Planning, 5=Modbus, 6=OCPP). *See Chapter 6 of STATE_MACHINE.md for details.* |
| `FM:X` | `FM:` | Sets the main Functioning Mode to `X` (e.g., `FM:1` for permanent direct charge). *See Chapter 6 of STATE_MACHINE.md for details.* |
| `FM2?` | `FM2:` | Queries the current state of the external signal (Dry Contact / TIC Peak/Off-Peak). |
| `FM2:1` / `FM2:0` | `FM2:` | Overrides the external signal. `FM2:0` simulates Peak Hours (suspends charge in `FM:2`). `FM2:1` simulates Off-Peak Hours (authorizes charge). |
| `SOK` / `SNOK` | `Slp:` | **Sleep**: `SOK` forces the station into deep sleep (`State:Y`). `SNOK` wakes the station up. |

---

## 6. OCPP & RFID
Commands managing interactions with user badges or Cloud supervision.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `OCPPPS?` | `OCPPPS:` | Requests the station's OCPP status (e.g., `OCPPStatus:Available`). |
| `OCPPPACOK` / `OCPPPACNOK` | `OCPPPAC:` | **TBD**: **P**lug **A**nd **C**harge: Enables/disables the badge-less "Plug and Charge" function. |
| `RFIDId:XXXXX` | `RFIDId:` | **TBD**: Transmits the RFID badge ID (read by the Pi) to the power board. |
| `RFIDA:XXXXX` | `RFIDA:` | **TBD**: Sends the RFID authorization status. |
| `OCPPCTO:XXX` | `OCPPCTO:` | **TBD**: Connection Time Out: Sets the connection timeout. |

---

## 7. Bluetooth Module (BLE)
The station has an integrated Bluetooth module (for the smartphone app) which can be toggled to avoid serial conflicts.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `BT?` | `BT:` | Requests the current state of the Bluetooth module (`BT:1` = On, `BT:0` = Off). |
| `BTOK` | `BT:1` | Orders the Bluetooth module to turn on. |
| `BTNOK`| `BT:0` | Orders the Bluetooth module to turn off. |

---

## 8. Spontaneous Telemetry (Emitted by ATmega)
The ATmega sends unsolicited messages over the serial line during specific events.

| Unsolicited Frame (RX) | Explanation |
| :--- | :--- |
| `RaspberryPi?` | Emitted spontaneously when the ATmega boots. It expects the Pi to answer `RaspberryPiModeOK` to confirm its presence. |
| `State:*` | Emitted spontaneously at each state machine transition (e.g., `State:A`, `State:B`). |
| `Start` | Emitted spontaneously at the start of a charging session. |
| `SBF:*` | Emitted spontaneously during physical hardware events. `SBF:1` is emitted when a vehicle is plugged in (which can override a previous `SBNOK` state), and `SBF:0` is emitted when the cable is removed. |
| `CC:XX` | The calculated current limit. Emitted at the beginning of a charge, and spontaneously sent every time it is recalculated (e.g., about every 10 seconds during a charge when TIC load balancing is active). |
| `CCI:X.XX` | *Current Instantaneous*. Emitted periodically while charging to report the actual current drawn by the vehicle (in Amperes). |
| `CPh:Mono` / `CPh:Tri` | *Charge Phases*. Emitted just before the charge ramps up to indicate the automatically detected number of phases used by the vehicle. |
| `\WT:...:CT:...:\` | **Session Ticket**. Emitted at the end of a charging session (when passing through states W then M). Example: `\WT:0:0:5:CT:0:6:58:EVplug:4485.20:0.00:\`. It contains Waiting Time (`WT`), Charging Time (`CT`), the socket used (`EVplug` or `DOMplug`), and the Average Power during Peak and Off-Peak hours. |
| `TICTestC:Init` / `TICTestB:X` | Spontaneous debugging frames emitted when TIC Test mode is enabled (`TICTM:1`), reporting baud rate and calculated limits. |
