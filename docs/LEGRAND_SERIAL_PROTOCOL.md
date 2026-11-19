# Legrand Serial Protocol (Green'up Power Board) — Complete Reference

> **Firmware Reference:** Microchip / Atmel **ATmega2560** running official Legrand firmware (`V01.19.02`, `V01.18.04`, `V01.17.77`).  
> **Physical Interface:** `USART1` at **115200 bauds, 8 data bits, no parity, 1 stop bit (8N1)**.  
> **Framing:** All commands sent to the power board must be terminated with a Carriage Return character (`\r`, ASCII `0x0D`). Line feed (`\n`) is ignored. Responses from the ATmega are terminated by `\r\n` (CRLF).  
>
> *Note 1:* The Green'up Link command API handles `\r` and `\n` framing characters autonomously; you do not need to add them manually.  
> *Note 2:* If a command is malformed or unrecognized, the ATmega responds with `Default:<Command>`.

---

## 🚦 Architectural Overview: Command Categories

The ATmega2560 serial command dispatcher bifurcates all incoming frames into two fundamental categories:
1. **Getters (Queries):** Commands ending with a question mark (`?`). Used to inspect internal registers, analog measurements, operating state, and configuration tables without side-effects.
2. **Setters & Actions:** Commands without `?` (e.g., `CC:16`, `Unlock`, `Reset`). Used to modulate power, simulate user interaction, toggle socket availability, or configure persistent parameters.

---

## 1. System Identification & Boot Handshake

Commands used to identify the hardware, negotiate supervision control, and manage system resets.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `RaspberryPiModeOK` | `DIP1:..` then `Initialized:` (1st call) / `Side:` then `DIP1:..` (subsequent) | **Supervision Handshake**: Declares the supervisor online and unlocks the full command API. On the first call, replies with `DIP1:<0/1>` through `DIP4:<0/1>` and `Initialized:<0/1>`. On subsequent calls, replies with `Side:<48/49>` and `DIP1:`..`DIP4:`. Note: `DIP1`..`DIP4` are reported in reverse order of the physical board switches (Switch 4 = `DIP1`, Switch 3 = `DIP2`, Switch 2 = `DIP3`, Switch 1 = `DIP4`). This handshake disables runtime polling of Switches 1 & 2, but does not override the boot-time current limit from Switches 3 & 4 (send `CCS:<val>` if needed). |
| `RaspberryPiMode?` | `RaspberryPiMode:0` | Queries supervisor status before handshake. Once `RaspberryPiModeOK` is active, this query is unrecognized (`Default:RaspberryPiMode`). |
| `KitCom?` | `KitCom:<0/1>` | **Supervisor Presence Query**: Returns `KitCom:1` if the handshake was completed, or `KitCom:0` in standalone mode. |
| `SoftwareVersion?` | `SoftwareVersion:` | Returns the application firmware version (e.g., `SoftwareVersion:V01.19.02`). |
| `HardwareVersion?` | `HardwareVersion:` | Returns the hardware board revision (e.g., `HardwareVersion:V02.01.01`). |
| `SerialNumber?` | `SerialNumber:` | Returns the unique 6-digit production serial number (e.g., `SerialNumber:123456`). |
| `Reference?` | `Reference:` | Returns the 6-digit commercial catalogue reference (e.g., `Reference:058001`). |
| `WeekYearProduction?`| `WeekYearProduction:`| Returns the manufacturing date code formatted as `<yy>W<ww>` or `<ww>W<yy>` (e.g., `WeekYearProduction:42W20`). |
| `Side?` | `Side:<48/49>` | Returns the socket side identifier as the decimal ASCII code of `'0'` (`48` = Left) or `'1'` (`49` = Right). |
| `Initialized?` | `Initialized:<0/1>` | Returns `Initialized:0` after a normal boot. Returns `Initialized:1` only if a factory reset was triggered at boot (5 s front-button hold during self-test), then resets to `0` once read. |
| `Reset` | `State:X` | **Hardware Reboot**: Executes a software reset, turns off front panel LEDs, emits `State:X`, and re-executes boot autotests. |
| `ping` | `pong` | Basic keepalive probe to test serial link responsiveness. |

---

## 2. Charge Telemetry, State Machine & Hardware Sensors

Queries reporting the real-time electrical state, IEC 61851-1 state transitions, and raw analog/digital sensor acquisitions.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `State?` | `State:` | Returns the current state of the main charging finite-state machine (`State:A` through `State:Z`). *See `STATE_MACHINE.md` for the complete transition diagram.* |
| `E?` | `E:` | Returns the current system error code (`E:0000` = No fault, `E:0001` to `E:0015` = Hardware faults such as lock timeout, diode missing, residual current, overcurrent, mains drop). |
| `CP?` | `CP:<volts>` | **Control Pilot Voltage**: Peak voltage measured on the CP line without units (`12` = Standby/Unplugged, `9` = Connected/Waiting, `6` = Charging, `3` = Ventilated charge, `0` = Disconnected/Fault). |
| `PP?` | `PP:<amps>` | **Proximity Pilot Current**: Sensed cable ampacity without units (`0` = No cable, `13`, `20`, `32`, `63`). |
| `CCCa?` | `CCCa:` | Cable Current Capacity: Query returning the maximum current allowed by the physical cable currently plugged in. |
| `CCI?` | `CCI:<L1>;<L2>;<L3>` | **Instantaneous Phase Currents**: Measures actual RMS current in Amperes. Always returns 3 semicolon-separated values (e.g., `CCI:15.98;0.00;0.00` in single-phase). |
| `FCS?` | `FCS:<0/1>` | **Full Charge State**: Returns `FCS:1` after 20 continuous minutes in `State:I` (vehicle suspended charge), or `FCS:0` otherwise. |
| `DOM?` | `DOM:<0/1>` | **Domestic Socket Sensor**: Reads the physical auxiliary micro-switch detecting whether a standard plug is inserted into the domestic Schuko socket (`1` = Present, `0` = Absent). |
| `SB?` | `SB:<0/1>` | **Start/Stop Button State**: Returns the current session run state (`1` = Active/Armed, `0` = Stopped/Paused). |
| `SBOK` / `SBNOK` | `SB:1` / `SB:0` (or `SBF:0` in `State:A`) | **Simulate Front Button**: `SBNOK` stops the session (transitions through `State:W` to `State:M`, closes the session, and unlocks T2S); `SBOK` arms/resumes the session. In `State:A`, both commands return `SBF:0`. |

---

## 3. Current & Power Modulation

Commands to query and modulate charging limits across hardware, supervisor, cloud, sub-line, and TIC layers.

> [!NOTE]
> The effective current setpoint (`CC`) applied to the vehicle's CP PWM signal is dynamically calculated by the ATmega as the minimum of active constraints depending on supervision status:
> * **With Supervisor (`RaspberryPiModeOK`):** $\min(\text{CCS}, \text{CCCa}, \text{CCTIC}, \text{CCEl}, \text{CCCo})$ *(capped to 16A if EV requests charge immediately at plug-in)*.
> * **Standalone (No Supervisor):** $\min(\text{CCS}, \text{CCCa}, \text{CCTIC}, \text{CCBLES}, [\text{CCBLESh in FM:4}])$.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `CC?` | `CC:<val>` | Queries the final consolidated charging limit currently applied to the vehicle (in Amperes). |
| `CC:<val>` | `CC:<val>` | **Direct Supervisor Current Limit (Volatile RAM)**: Sets the volatile supervisor setpoint in RAM (2-digit format `XX` required, e.g., `CC:16`), with **zero EEPROM wear**. Recalculates PWM immediately if charging is active (`C`/`D`/`E`/`I`, emits `CC:<effective>`). Reset to `CCS` on reboot or `FM:x` mode changes. Recommended method for dynamic power modulation. |
| `CCS?` | `CCS:<val>` | **Station / Socket Current Limit**: Queries the station maximum current limit (initialized at boot from DIP switches 3 & 4: 16A, 20A, 25A, or 32A). |
| `CCS:<val>` | `CCS:<val>` | Sets the station maximum current limit. |
| `CCEl?` | `CCEl:<val>` | Queries the Eco-Logic / Eliot Current limit. |
| `CCEl:<val>` | `CCEl:<val>` | **Eco-Logic / Eliot Current Limit (Persistent EEPROM)**: Writes the current limit into non-volatile EEPROM via `EEPROM.put` (`EERef::update`, e.g., `CCEl:16`). Never overwritten by background routines, but **avoid for dynamic modulation** due to ATmega2560 EEPROM wear (~100,000 cycles). |
| `CCCo?` | `CCCo:<val>` | Queries the volatile supervisor setpoint register (the value written by `CC:<val>` or Modbus load-shedding). |
| `CCTIC?` | `CCTIC:<val>` | Queries the dynamic current cap calculated from the Enedis Linky TIC input. Returns `CCS` when TIC is absent or outside an active session. |
| `CCSL1?` .. `CCSL4?`| `CCSL1:` .. `CCSL4:` | **Sub-Line / Cluster Current Levels**: Queries the 4 current step levels in descending order (`CCSL1` = maximum/`CCS`, `CCSL4` = minimum). |
| `CCBLES?` | `CCBLES:` | Queries the hardware/custom current limit (used in standalone mode). |
| `CCBLESh?` | `CCBLESh:` | Queries the scheduled/Modbus limit current setpoint (used in standalone `FM:4`). |

---

## 4. Sockets & Actuator Hardware Control

Commands to independently inspect, enable, force, or unlock the power receptacles.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `T2C?` | `T2C:<0/1>` | Queries whether the primary **Type 2 (T2S)** socket is enabled (`1`) or disabled (`0`). |
| `T2COK` / `T2CNOK` | `T2C:1` / `T2C:0` | **Enable / Disable Type 2 Socket**: `T2COK` authorizes charging on Type 2; `T2CNOK` inhibits charging. |
| `2PC?` | `2PC:<0/1>` | Queries whether the secondary **Domestic (Schuko / 2P+T)** socket is enabled (`1`) or disabled (`0`). |
| `2PCOK` / `2PCNOK` | `2PC:1` / `2PC:0` | **Enable / Disable Domestic Socket**: `2PCOK` authorizes charging on Schuko; `2PCNOK` software-inhibits the domestic socket. *(Note: The Type 2 socket always takes absolute priority over the domestic socket; plugging into T2S cuts any ongoing domestic charge immediately).* |
| `T2F?` | `T2F:<0/1>` | Queries whether charging is currently forced on the Type 2 socket. |
| `T2FOK` / `T2FNOK` | `T2F:1` / `T2F:0` | **Force Type 2 Socket**: Overrides schedule/standby to force power onto the Type 2 contactor. |
| `2PF?` | `2PF:<0/1>` | Queries whether charging is currently forced on the domestic socket. |
| `2PFOK` / `2PFNOK` | `2PF:1` / `2PF:0` | **Force Domestic Socket**: Overrides schedule/standby to force power onto the domestic relay. |
| `Unlock` | `Unlock` then `State:A` (or `E:0002` + `State:R`) | **Cable Unlock**: If charging (`State:D`/`State:E`), cleanly stops the charge (CP to +12V, emits `State:W`, waits up to 6.1 s for the vehicle to drop 6V, opens contactor, closes session), fires a 120 ms unlock pulse to retract the T2S pin, and emits `Unlock` followed by `State:A` (or `E:0002` + `State:R` if mechanically jammed). |

---

## 5. Planning, Scheduling, Operating Modes & Derogation

Commands to inspect and configure deferred charging, operating modes, schedules, and derogations.

> [!NOTE]
> **Weekly Schedule Calendar (Firmware V01.19.02 Specific):**  
> Starting with version `V01.19.02`, Legrand embedded a full weekly charging calendar directly into the ATmega firmware.  
> In a quirk of implementation, the firmware accepts **both** the capitalized and lowercase query forms: `Calendar?` and `calendar?`. Both output an ASCII timetable showing scheduled start and stop hours.  
> **Operational Impact**: This internal calendar is purely read-only via serial console and **is evaluated only in Mode 4 (`FM:4`)**. In modes `FM:1`, `FM:2`, `FM:5`, and `FM:6`, this calendar is completely ignored by the state machine.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `Calendar?` / `calendar?` | `start ... stop ...` | **Weekly Schedule Table**: Emits a formatted 7-day ASCII schedule grid (Mon through Sun) showing programmed charge start and stop windows. Unprogrammed slots are printed as blank spaces. Ignored unless the station operates in `FM:4` (standalone). |
| `PA?` | `PA:<0/1>` | **Planning Authorization**: Queries whether charge authorization according to the weekly calendar planning is currently active. |
| `PAOK` / `PANOK` | `PA:1` / `PA:0` | **Set Planning Authorization**: `PAOK` enables adherence to the planned charging schedule; `PANOK` disables it. |
| `PAFC?` | `PAFC:<0/1>` | **Planning Auto Force Charge**: Queries whether automatic forced charging is enabled to override schedule windows. |
| `PAFCOK` / `PAFCNOK`| `PAFC:1` / `PAFC:0` | **Set Planning Auto Force Charge**: `PAFCOK` forces charging even during schedule off-peak/blocked windows; `PAFCNOK` respects schedule boundaries. |
| `D?` | `D:<0/1>` | **Derogation Query**: Checks if manual derogation is active (`1` = Active, `0` = Inactive). |
| `DOK` / `DNOK` | `D:1` / `D:0` | **Toggle Derogation**: Activates (`DOK`) or deactivates (`DNOK`) temporary derogation. Internally evaluated in logical OR with the physical Off-Peak dry contact (Contact 1) and with the Linky TIC Off-Peak flag (`Contact_HC \|\| TIC_HC \|\| Derogation`). `DOK` immediately overrides a Peak-Hours (HP) Linky lockout or `FM2:1` conditioning. |
| `Dec?` | `Dec:<min>` | **Deferred Start Delay Query**: Returns remaining delay time in minutes before deferred charging starts. |
| `Dec:<min>` | `Dec:<min>` | **Set Deferred Start Delay** *(Mode `FM:1` only)*: Configures deferred charging delay quantized in 3-hour steps (`0`, `180` for 1..180, `360` for 181..360, `540` for 361..540; values > 540 are ignored). Ignored outside `FM:1` (returns `Dec:0`). When the timer expires, charging starts if authorized (still blocked if Linky TIC is in Peak Hours without `DOK`). |
| `Slp?` | `Slp:<0/1>` | **Sleep Query**: Checks if the station is currently in sleep mode (`1` if `State:Y`). |
| `SOK` / `SNOK` | `Slp:1` then `State:Y` / `Slp:0` then `State:A` | **Sleep Mode Toggle**: `SOK` stops any active charge (via `State:W`), unlocks T2S, and enters sleep mode (`State:Y`); `SNOK` wakes the station and returns to `State:A`. |
| `FM?` | `FM:<val>` | **Functioning Mode Query (Dynamic State Calculation)**: Dynamically computes the active mode from internal state flags:<br>• **`FM:1`**: Direct Mode (Nominal) — returned when no conditioning, schedule, Modbus, or OCPP flag is active.<br>• **`FM:2`**: Contact Conditioning Active — returned when `FM2:1`, `FM2:2`, or `FM2:3` is active.<br>• **`FM:4`**: Weekly Calendar / Planning mode (Bluetooth schedule).<br>• **`FM:5`**: Modbus RTU Slave mode (RS485 `Serial3`).<br>• **`FM:6`**: Remote OCPP Cloud supervisor.<br>*Note: Mode 3 does not exist; sending `FM:3` falls back to `FM:1`.* |
| `FM:<val>` | `FM:<val>` | Requests switching operating mode profile (`FM:1` recommended for `greenup-link`). Mode `FM:3` aliases to `FM:1`. Note: sending `FM:2` when `FM2:0` is set outputs `FM:2` / `FM2:0`, leaving `FM?` at `1` until `FM2:1..3` is set. |
| `FM2?` | `FM2:<0..3>` / `FM2F:0` | **Forcing Sub-Mode Query**: Returns the current 2-bit dry contact conditioning mask. In `FM:4`, `FM:5`, or `FM:6`, clears conditioning and returns `FM2F:0`. |
| `FM2:<0..3\|x>` | `FM2:<val>` / `FM2F:0` | **Forcing Sub-Mode (2-Bit Mask)**:<br>• **`FM2:0` (`00`)** or **`FM2:x`**: Direct / Nominal Mode (charges in `State:E` / `State:H`). `FM2:x` replies `FM2F:0`.<br>• **`FM2:1` (`01`)**: Contact 1 Conditioning (Off-Peak simulation / HP lock; charges in `State:D` / `State:G`). Ideal for `greenup-link` to inhibit automatic charge on plug-in until `DOK` or contact closure.<br>• **`FM2:2` (`10`)**: Contact 2 conditioning (not overridable by `DOK`).<br>• **`FM2:3` (`11`)**: Combined Contact 1 and Contact 2 conditioning. |
| `CAU?` | `CAU:<0/1>` | **Charge Authorization Query — DO NOT USE**: Checks whether both T2S and Domestic authorization flags are aligned. If they differ (which occurs during TIC load-shedding or single-socket sessions), `CAU?` resets both to `0` and **abruptly cuts off active charging**. |
| `CAUOK` / `CAUNOK` | `CAU:1` / `CAU:0` | **Charge Authorization Switch**: `CAU` stands for **Charge AUthorization**. `CAUNOK` cleanly stops charging through deceleration state `State:W` (PWM to 0A), opens the contactor off-load, and holds the vehicle plugged/locked in **`State:B`**. `CAUOK` restores authorization. *(In `State:D`, `CAUNOK` requires `RaspberryPiModeOK` to be active).* |

---

## 6. Modbus RS485 & Energy Management Telemetry

Commands to monitor active energy consumption and configure external RS485 bus parameters (Energy Meters and Dynamic Load Management units).

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `EM?` | 3 lines: `EM:<0\|1>XX`, `EM:X<0\|1>X`, `EM:XX<0\|1>` | **Energy Management Mask / Hardware Inputs**: Returns 3 separate lines:<br>• `EM:<0\|1>XX`: Contact 2 (ON/OFF input).<br>• `EM:X<0\|1>X`: Contact 1 (HP/HC input).<br>• `EM:XX<0\|1>`: Manual derogation state (`DOK` / > 1 s front-button press).<br>*Spontaneous push notifications (`EM:...`) are emitted only when `FM:2` (`FM2:1..3`) and `RaspberryPiModeOK` are active.* |
| `ModbusAddr?` | `ModbusAddr:<addr>` | Queries the station's slave address on the Modbus RS485 bus (default: `1`). |
| `ModbusAddr:<addr>` | `ModbusAddr:<addr>` | Configures the station's Modbus slave address in EEPROM and echoes the value. |
| `ModbusEMA?` | `ModbusEMA:<0/1>` | Queries whether the external Modbus Energy Meter (fixed at Modbus address `5`) is enabled (`1`) or disabled (`0`). |
| `ModbusEMA:<0/1>` | `ModbusEMA:<0/1>` | Enables (`1`) or disables (`0`) polling of the external Energy Meter (address `5`). Enabling disables Linky TIC reception. |
| `ModbusDLMA?` | `ModbusDLMA:<0/1>`| Queries whether Dynamic Load Management (DLM) via Modbus is enabled (`1`) or disabled (`0`). |
| `ModbusDLMA:<0/1>`| `ModbusDLMA:<0/1>` | Enables (`1`) or disables (`0`) Modbus DLM in EEPROM. |
| `ModbusP?` | `ModbusP:<0/1/2>` | Queries Modbus parity (`0` = None, `1` = Even, `2` = Odd). |
| `ModbusP:<0/1/2>` | `ModbusP:<0/1/2>` | Sets Modbus parity in EEPROM and echoes the value. |
| `ModbusB?` | `ModbusB:<baud>` | Queries Modbus baud rate index (`0`..`8`). |
| `ModbusB:<baud>` | `ModbusB:<baud>` | Sets Modbus baud rate index in EEPROM and echoes the value. |
| `ModbusSB?` | `ModbusSB:<0/1>` | Queries Modbus stop bits setting (`0` = 1 stop bit, `1` = 2 stop bits). |
| `ModbusSB:<0/1>` | `ModbusSB:<val>` | Configures Modbus stop bits (`0` = 1 bit, `1` = 2 bits) in EEPROM. *(Firmware quirk: due to a string offset bug, the setter parses from the colon character and always evaluates to `0`).* |

---

## 7. Access Control & RFID Badges

Commands managing user authentication, RFID reader hardware, and OCPP sessions.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `RFIDE?` | `RFIDE:<0/1>` | Queries whether the RFID badge reader peripheral is enabled. |
| `RFIDEOK` / `RFIDENOK` | `RFIDE:1` / `0` (then `RFIDAF:2`) | Enables (`RFIDEOK`) or disables (`RFIDENOK`) the RFID badge reader module. |
| `RFIDUM?` | `RFIDUM:<0/1>` | **RFID User Management Mode**: Checks whether RFID access control is local (`0`) or delegated to the supervisor (`1`). |
| `RFIDUMOK` / `RFIDUMNOK` | `RFIDUM:1` / `RFIDUM:0` | Enables (`RFIDUMOK`) or disables (`RFIDUMNOK`) online supervisor badge validation. |
| `RFIDA?` | `RFIDA:<0..5>` | Queries the current RFID authorization state (`0`=Refused, `1`=Accepted, `2`=Idle, `3`=Reset, `4`=Stop with cable locked, `5`=Pending). |
| `RFIDA:<0..5>` | `RFIDA:<val>` / `RFIDAF:<val>` | Sets the RFID authorization state (`0`=Refuse, `1`=Accept, `2`=Idle, `3`=Reset/Cancel, `4`=Stop keeping cable locked in `State:K`/`State:O`, `5`=Pending animation). |
| `RFIDId:<hex>` | `RFIDId:<hex>` | Transmits a badge UID (hex string) to validate authorization and store it in the active badge registers. |
| `RFIDIdLP:<hex>` | `RFIDIdLP:<hex>` | Transmits a badge UID (hex string) read in `State:A` to trigger badge presentation handling. |
| `OCPPPS?` | `OCPPStatus:` | **OCPP Power Status**: Returns the standardized OCPP 1.6 status: `Available`, `Preparing-T2S`, `Preparing-DOM`, `Charging`, `SuspendedEVSE`, `SuspendedEV`, `Faulted`, or `Unavailable`. |
| `OCPPPACOK` / `NOK` | *(no reply)* | **OCPP Plug and Charge**: Enables or disables plug-and-charge handling under OCPP supervision (produces no serial reply). |
| `OCPPCTO:<ms>` | `OCPPCTO:<ms>` | **OCPP Connection Timeout**: Configures the badge-to-plug connection timeout window in **milliseconds**. |

---

## 8. Bluetooth Controller (BLE)

Controls the integrated Bluetooth transceiver (used for mobile pairing) to eliminate bus contention.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `BT?` | `BT:<0/1>` | Returns `BT:1` if the Bluetooth module is powered and active, or `BT:0` if powered off. |
| `BTOK` | `BT:1` | Powers on and initializes the Bluetooth module. |
| `BTNOK` | `BT:0` | Cuts power to the Bluetooth module to avoid RF or serial contention. |
| `MacAdr?` | `MacAdr:<12-hex>` | Returns the 12-character MAC address of the Bluetooth radio when active (`BT:1`), or twelve zeros when off (`MacAdr:000000000000`). *Note: In patched firmware builds, this vector returns Linky TIC diagnostic flags.* |

---

## 9. Diagnostic & Test Modes (`*TM` Commands)

> [!TIP]
> **Debugging Use Only:**  
> All commands ending with **`TM` (Test Mode)** are intended strictly for diagnostic probing. Each command echoes its state (`<Cmd>:<0/1>`) followed by an initialization line (`TICTestC:Init`, `ModbusTest:Init`, `BLETest:Init`, `DryContactP:Init`). Leave disabled (`0`) in standard operation.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `TICTM:<0/1>` | `TICTM:<0/1>` then `TICTestC:Init` | **TIC Test Mode**: Outputs detected TIC baud rate (`TICTestB:<baud>`) in `State:A` and the calculated TIC available current (`TICTestC:<amps>`) on decoded groups. |
| `RS485TM:<0/1>` | `RS485TM:<0/1>` then `ModbusTest:Init` | **RS485 Test Mode**: Dumps raw hex packets (`ModbusTest: ...`) exchanged over the RS485 Modbus bus. |
| `BLETM:<0/1>` | `BLETM:<0/1>` then `BLETest:Init` | **Bluetooth Test Mode**: Dumps raw hex traffic (`BLETest: ...`) exchanged with the Bluetooth radio module. |
| `DRYCTCTM:<0/1>` | `DRYCTCTM:<0/1>` then `DryContactP:Init` | **Dry Contact Test Mode**: Outputs `DryContactP:HP/HC ...+ON/OFF ...` once per second. |

---

## 10. Factory Configuration & Calibration

> [!CAUTION]
> **CRITICAL WARNING — PERSISTENT EEPROM WRITES:**  
> These parameters directly control firmware hardware branching at boot (phase count, current limits, ADC calibration, and TIC UART routing—e.g. hardware `Serial3` TIC is only active on `V02.01.xx` boards).  
> **Never modify these parameters unless replacing a board or executing factory recalibration.**

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `Reference:<ref>` | `Reference:<ref>` *(or `<ref>*` in `prodMode`)* | Writes the 6-digit product reference catalogue code to EEPROM. |
| `HardwareVersion:<6-digits>`| `HardwareVersion:Vxx.yy.zz` | Writes the 6-digit hardware revision code (sent without `V` or dots, e.g., `020101`) to EEPROM. |
| `WeekYearProduction:<4-digits>`| `WeekYearProduction:<xxWyy>` | Writes the 4-digit manufacturing date code (sent without `W`, e.g., `4220`) to EEPROM. |
| `SerialNumber:<sn>` | `SerialNumber:<sn>` | Writes the 6-digit serial number to EEPROM. |
| `Side:<0/1>` | `Side:<48/49>` | Configures the socket side identifier (`0` = Left, `1` = Right) in EEPROM. |
| `coefPh1..3:<val>` | `<val>*` *(prodMode only)* | *(Production bench mode only)* Calibrates Phase 1..3 current ADC multiplier factors in EEPROM. Not available in supervisor mode. |
| `SoftwareVersion:<6-digits>` | `<ver>*` *(prodMode only)* | *(Production bench mode only)* Writes the 6-digit software version to EEPROM. Not available in supervisor mode. |
| `UpdateAuth:<code>` | `UpdateCodeOK` / `UpdateCodeNOK` | **Firmware Update Authorization**: Validates the update authorization code before entering bootloader update mode. |

---

## 11. Production & Service Test Modes (`service`, `State:Z`)

> [!WARNING]
> **HIGH RISK — FACTORY BENCH ONLY:**  
> The Production Mode (`prodModeOK` $\rightarrow$ `State:Z`) and `service` menu are dedicated exclusively to automated factory end-of-line testing benches.  
> **Entering State `'Z'` bypasses all normative IEC 61851-1 safety interlocks** and disables the watchdog timer.  
> **Executing these commands while a vehicle or human is connected presents severe shock and equipment hazards.**

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `service` | `---service menu---` | Launches the interactive text diagnostic menu on `Serial1`. Disables watchdog. |
| `prodModeOK` | `prodMode:OK*` | **Enters State 'Z' (Factory Test Bench)**: Switches the FSM to State `'Z'`, enabling raw bench test commands. |
| `prodModeNOK` | `prodMode:NOK*` | **Exits State 'Z'**: Re-arms standard safety watchdogs and returns the FSM to State `'A'`. |
| `testled` | *(none)* | *(State 'Z' only)* Cycles through Red, Green, and Blue LEDs. |
| `testbouton` | `B:ok*` / `B:nok*` | *(State 'Z' only)* Tests front panel button response. |
| `testcontacteurdomplug` | `C:O*` / `C:F*` | *(State 'Z' only)* Manually cycles the auxiliary domestic socket relay (advance with `C:suivant`). |
| `testcontacteurevplug` | `D:ok*` / `DO:nok*` / `DF:nok*` | *(State 'Z' only)* **DANGER**: Manually cycles the 32A main power contactors. |
| `testbobineaemission` | `E:O*` / `E:F*` | *(State 'Z' only)* **DANGER**: Pulses the 230V Shunt Trip coil (advance with `E:suivant`). |
| `testmoteur` | `F:ok*` / `FD:nok*` / `FV:nok*` | *(State 'Z' only)* Cycles the motorized socket lock mechanism back and forth. |
| `testalimentation` | `G:ok*` / `G12V:nok*` / `G230V:nok*` | *(State 'Z' only)* Verifies 230V AC mains presence sensor. |
| `testCP:<12V..0V>` | `ok:<val>*` / `nok:<val>*` | *(State 'Z' only, after `testCP`)* Validates CP ADC voltage thresholds (`12V`, `9V`, `6V`, `3V`, `0V`). |
| `testPP:<13A..63A>` | `ok:<val>*` / `nok:<val>*` | *(State 'Z' only, after `testPP`)* Validates PP resistor thresholds (`13A`, `20A`, `32A`, `63A`). |
| `test6mA` | `test6mA:OK*` / `test6mA:NOK*` | *(State 'Z' only)* Triggers self-test on the internal 6mA DC residual current sensor. |
| `testbluetooth` | `R:ok*` / `R:nok*` | *(State 'Z' only)* Tests Bluetooth radio configuration and communication. |

---

## 12. Spontaneous Telemetry (Emitted Unsolicited by ATmega)

The ATmega2560 broadcasts asynchronous notifications on `TX1`:

| Frame (RX) | Transmission Trigger | Explanation |
| :--- | :--- | :--- |
| `RaspberryPi?` | System Boot | Emitted once at the end of boot initialization to probe for supervisor readiness. |
| `Start` | Session Open | Emitted when a new session opens from `State:A` (vehicle plugged into T2S before `State:B`, or plug inserted into domestic socket before `State:F`). |
| `State:<A..Z>` | State Transition | Emitted immediately whenever the charging state machine transitions (e.g., `State:B`, `State:C`, `State:W`, `State:F`, `State:H`). |
| `CCCa:<amps>` / `CCS:<amps>` | T2S Plug-In | Emitted at T2S plug-in in `State:A` just before `SBF:1` and `Start`. |
| `CC:<amps>` | Dynamic Limit Change | Broadcast automatically whenever the effective charging current limit changes (e.g., following `CCEl`, `CCTIC`, `CCCa`, or cable connection). |
| `CCI:<amps>` | During Active Charge | Broadcast every second while charging (`CCI:<L1>` in single-phase, `CCI:<L1>;<L2>;<L3>` in three-phase; also emitted during domestic socket charge via Phase 1 CT). |
| `CPh:Mono` / `CPh:Tri` | During Active Charge | Emitted at the 1st second of charge and repeated every 60 s to report active phase topology (`Mono` or `Tri`). |
| `FCS:1` | 20 min in `State:I` | Emitted after 20 continuous minutes in `State:I` (vehicle suspended charge). |
| `Dec:<min>` / `Dec:0` | Deferred Start | Emitted on front-button release when adjusting deferred delay, on timer expiration, or at session close. |
| `EM:<mask>` | Input Change (`FM:2` + `RaspberryPiModeOK`) | Emitted spontaneously only in `FM:2` (`FM2:1..3`) with supervisor active: `EM:1XX`/`0XX` (Contact 2), `EM:X1X`/`X0X` (Contact 1), `EM:XX1`/`XX0` (Derogation). |
| `E:<0000..0015>` | Error Event / Boot | Emitted upon boot (`E:0000`) and whenever a hardware fault occurs or clears. |
| `SBF:1` / `SBF:0` | Session Start / Stop / Button | Signals that `SB` has been forced to `1` (plug-in / start) or `0` (unplug / stop / short front-button press). |
| `FM2F:0` | Forced Conditioning Clear | Emitted when `FM2:x` is sent or when `FM2?` is queried in `FM:4`/`FM:5`/`FM:6`. |
| `WT:<h>:<m>:<s>:CT:<h>:<m>:<s>:<EVplug\|DOMplug>:<HP_W>:<HC_W>:` | Charge Session End | **Session Ticket**: Summarizes Waiting Time (`WT`), Charging Time (`CT`), socket (`EVplug` or `DOMplug`), and average power in Watts during Peak (`HP`) and Off-Peak (`HC`) periods as determined by the physical Contact 1 input (not Linky TIC). |
| `RFIDAF:<2\|3\|4>` / `OCPPStatus:<...>` / `OCPPPAC:<0\|1>` | OCPP / RFID Modes | Emitted only when RFID (`RFIDEOK`) or OCPP Cloud mode (`FM:6`) is active. |
| `Freq:` / `Volt:` / `DLMCI:` / `Ener:` / `MP:` | External Modbus Meter | Emitted every second only when an external Modbus Energy Meter (`ModbusEMA:1`) is enabled and responding. |
