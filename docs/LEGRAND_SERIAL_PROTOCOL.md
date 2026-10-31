# Legrand Serial Protocol (Green'up Power Board) — Complete Reference

> **Firmware Reference:** Microchip / Atmel **ATmega2560** running official Legrand firmware (`V01.19.02`, `V01.18.04`, `V01.17.77`).  
> **Physical Interface:** `USART1` (Pins `PD2`/RXD1 and `PD3`/TXD1) at **115200 bauds, 8 data bits, no parity, 1 stop bit (8N1)**.  
> **Framing:** All commands sent to the power board must be terminated with a Carriage Return character (`\r`, ASCII `0x0D`). Line feed (`\n`) is ignored. Responses from the ATmega are terminated by `\r\n` (CRLF).  
>
> *Note 1:* In the `cli_greenup-link.ps1` terminal or REST API, you do not need to append `\r`; the software layer adds it automatically.  
> *Note 2:* If a command is malformed or unrecognized, the ATmega responds with `Default:<Command>`.

---

## 🚦 Architectural Overview: Command Categories

The ATmega2560 serial command dispatcher (`0x0AD12` in Flash) bifurcates all incoming frames into two fundamental categories:
1. **Getters (Queries):** Commands ending with a question mark (`?`). Used to inspect internal registers, analog measurements, operating state, and configuration tables without side-effects.
2. **Setters & Actions:** Commands without `?` (e.g., `CC:16`, `Unlock`, `Reset`). Used to modulate power, simulate user interaction, toggle socket availability, or configure persistent EEPROM parameters.

---

## 1. System Identification & Boot Handshake

Commands used to identify the hardware, negotiate supervision control, and manage system resets.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `RaspberryPiModeOK` | `Side:` then `DIP1:..` | **Supervision Handshake Magic Frame**: Informs the power board that the Raspberry Pi supervisor is online. Arms `g_kit_com_present = 1` (SRAM `0x0DB0`), unlocking the full command API and delegating power/access control. The ATmega acknowledges by sending `Side:<1/2>`, physical switch states `DIP1:..DIP4:`, and `Initialized:0`. |
| `KitCom?` | `KitCom:<0/1>` | **Supervisor Presence Query**: Returns `KitCom:1` if the handshake was successfully completed, or `KitCom:0` if operating in standalone autonomous mode. |
| `SoftwareVersion?` | `SoftwareVersion:` | Returns the application firmware version (e.g., `SoftwareVersion:V01.19.02`). |
| `HardwareVersion?` | `HardwareVersion:` | Returns the hardware board revision stored in EEPROM (e.g., `HardwareVersion:V02.01.01`). |
| `SerialNumber?` | `SerialNumber:` | Returns the unique 6-digit production serial number (e.g., `SerialNumber:123456`). |
| `Reference?` | `Reference:` | Returns the 6-digit commercial catalogue reference (e.g., `Reference:058001`). |
| `WeekYearProduction?`| `WeekYearProduction:`| Returns the manufacturing date code (e.g., `WeekYearProduction:W4220` for week 42, year 2020). |
| `Side?` | `Side:` | Returns the active charge point socket number (`Side:1` or `Side:2`). |
| `Initialized?` | `Initialized:<0/1>` | Returns `Initialized:1` once the power board has completed its boot autotests (mains presence, lock cycle, CP PWM 100%). |
| `Reset` | `State:X` | **Hardware Reboot**: Arms `g_reset_requested = 0xFF`, emits `State:X`, turns off front panel LEDs, and executes an absolute assembly jump to address `0x00000` (`eicall 0x0000` — Reset Vector 0). Clears all BSS variables in SRAM and re-executes boot autotests. |
| `ping` | `pong` | Basic keepalive probe to test serial link responsiveness. |

---

## 2. Charge Telemetry, State Machine & Hardware Sensors

Queries reporting the real-time electrical state, IEC 61851-1 state transitions, and raw analog/digital sensor acquisitions.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `State?` | `State:` | Returns the current state of the main charging finite-state machine (`State:A` through `State:Z`). *See `STATE_MACHINE.md` for the complete transition diagram.* |
| `E?` | `E:` | Returns the current system error code (`E:0000` = No fault, `E:0001` to `E:0015` = Hardware faults such as lock timeout, diode missing, residual current, overcurrent, mains drop). |
| `CP?` | `CP:` | **Control Pilot Voltage**: Raw peak voltage measured on the CP line (`12V` = Standby/Unplugged, `9V` = Connected/Waiting, `6V` = Charging, `3V` = Ventilated charge, `0V` = Disconnected/Fault). |
| `PP?` | `PP:` | **Proximity Pilot Current**: Sensed cable ampacity derived from the resistor coding (`0A` = No cable, `13A`, `20A`, `32A`, `63A`). |
| `CCCa?` | `CCCa:` | Cable Current Capacity: Query returning the maximum current allowed by the physical cable currently plugged in. |
| `CCI?` | `CCI:` | **Instantaneous Phase Currents**: Measures actual current drawn across the active lines in Amperes (`CCI:L1` in single-phase; `CCI:L1;L2;L3` in three-phase, e.g., `CCI:15.98;16.02;15.95`). |
| `FCS?` | `FCS:<0/1>` | **Full Charge State**: Returns `FCS:1` when the connected vehicle's battery management system (BMS) has reached 100% capacity and terminated active current drawing, or `FCS:0` otherwise. |
| `DOM?` | `DOM:<0/1>` | **Domestic Socket Sensor**: Reads the physical auxiliary contact on pin `D28` detecting if a standard plug is inserted into the domestic Schuko socket (`1` = Present, `0` = Absent). |
| `SB?` | `SB:<0/1>` | **Stop Button State**: Returns the current logical state of the front panel capacitive touch / push button (`1` = Pressed / Pause active, `0` = Released / Active). |
| `SBOK` / `SBNOK` | `SB:1` / `SB:0` | **Simulate Front Button**: Remotely simulates pressing (`SBNOK` -> Pause charge, transitions to `State:M`) or releasing (`SBOK` -> Resume charge cycle) the front panel button. |

---

## 3. Current & Power Modulation

Commands to query and modulate charging limits across hardware, supervisor, cloud, sub-line, and TIC layers.

> [!NOTE]
> The effective current setpoint (`CC`) applied to the vehicle's CP PWM signal is dynamically calculated by the ATmega as the minimum of all active constraints:
> $$\text{Consigne Effective} = \min(\text{DIP/Hardware}, \text{CCCa}, \text{CCS}, \text{CCEl}, \text{CCTIC}, \text{CCCo})$$

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `CC?` | `CC:` | Queries the current consolidated active charging limit enforced on the vehicle (in Amperes). |
| `CC:<val>` | `CC:<val>` | **Direct Current Override**: Forces the charging current setpoint (format `XX` recommended, e.g., `CC:16`). Recalculates PWM on Timer 3 (`D5`) immediately if charging is active. |
| `CCS?` | `CCS:` | Queries the Supervisor Current limit (stored in SRAM `0x032A`). |
| `CCS:<val>` | `CCS:<val>` | Sets the Supervisor Current limit (e.g., `CCS:20`). |
| `CCEl?` | `CCEl:` | Queries the Cloud Eliot Current limit (stored in SRAM `0x1235`). |
| `CCEl:<val>` | `CCEl:<val>` | Sets the Cloud Eliot Current limit (e.g., `CCEl:16`). Safest method for remote automation. |
| `CCCo?` | `CCCo:` | Queries the internal Consolidated Current setpoint (SRAM `0x1238`). |
| `CCTIC?` | `CCTIC:` | Queries the dynamic current cap calculated from the Enedis Linky TIC input. Returns `32` if TIC is absent or idle. |
| `CCSL1?` .. `CCSL4?`| `CCSL1:` .. `CCSL4:` | **Sub-Line Current Limits**: Queries the dynamic power-sharing quotas for cluster sub-lines 1 through 4 (used in multi-station master/slave load shedding topologies). |
| `CCBLES?` | `CCBLES:` | Queries the Bluetooth Standard current setpoint (SRAM `0x1237`). |
| `CCBLESh?` | `CCBLESh:` | Queries the Bluetooth Shunt current setpoint (SRAM `0x1236`). |

---

## 4. Sockets & Actuator Hardware Control

Commands to independently inspect, enable, force, or unlock the power receptacles.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `T2C?` | `T2C:<0/1>` | Queries whether the primary **Type 2 (T2S)** socket is enabled (`1`) or disabled (`0`). |
| `T2COK` / `T2CNOK` | `T2C:1` / `T2C:0` | **Enable / Disable Type 2 Socket**: `T2COK` authorizes charging on Type 2; `T2CNOK` inhibits charging. |
| `2PC?` | `2PC:<0/1>` | Queries whether the secondary **Domestic (Schuko / 2P+T)** socket is enabled (`1`) or disabled (`0`). |
| `2PCOK` / `2PCNOK` | `2PC:1` / `2PC:0` | **Enable / Disable Domestic Socket**: `2PCOK` authorizes charging on Schuko; `2PCNOK` locks the socket out. |
| `T2F?` | `T2F:<0/1>` | Queries whether charging is currently forced on the Type 2 socket. |
| `T2FOK` / `T2FNOK` | `T2F:1` / `T2F:0` | **Force Type 2 Socket**: Overrides schedule/standby to force power onto the Type 2 contactor. |
| `2PF?` | `2PF:<0/1>` | Queries whether charging is currently forced on the domestic socket. |
| `2PFOK` / `2PFNOK` | `2PF:1` / `2PF:0` | **Force Domestic Socket**: Overrides schedule/standby to force power onto the domestic relay (`D31`). |
| `Unlock` | `Unlock:ok*` / `nok*` | **Emergency Cable Unlock**: Fires a 120 ms pulse into the H-bridge motor driver (`D44`..`D46`) to physically retract the locking pin on the T2S socket. Refused (`Unlock:nok*`) if current is actively flowing (`State:C`/`State:D`). |

---

## 5. Planning, Scheduling & Weekly Calendar

Commands to inspect and configure deferred charging, operating modes, and schedules.

> [!NOTE]
> **Weekly Schedule Calendar (Firmware V01.19.02 Specific):**  
> Starting with version `V01.19.02`, Legrand embedded a full weekly charging calendar directly into the ATmega firmware (SRAM table `0x123E`–`0x1275`, 7 days $\times$ 8 bytes).  
> In a quirk of implementation, the firmware accepts **both** the capitalized and lowercase query forms: `Calendar?` and `calendar?`. Both output an ASCII timetable showing scheduled start and stop hours.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `Calendar?` / `calendar?` | `start ... stop ...` | **Weekly Schedule Table**: Emits a formatted 7-day ASCII schedule grid (Mon through Sun) showing programmed charge start and stop windows. Value `-1` indicates an unprogrammed day slot. *(Exclusive to V01.19.02+)*. |
| `PA?` | `PA:<0/1>` | **Planning Authorization**: Queries whether charge authorization according to the weekly calendar planning is currently active. |
| `PAOK` / `PANOK` | `PA:1` / `PA:0` | **Set Planning Authorization**: `PAOK` enables adherence to the planned charging schedule; `PANOK` disables it. |
| `PAFC?` | `PAFC:<0/1>` | **Planning Auto Force Charge**: Queries whether automatic forced charging is enabled to override schedule windows. |
| `PAFCOK` / `PAFCNOK`| `PAFC:1` / `PAFC:0` | **Set Planning Auto Force Charge**: `PAFCOK` forces charging even during schedule off-peak/blocked windows; `PAFCNOK` respects schedule boundaries. |
| `D?` | `D:<0/1>` | **Derogation Query**: Checks if manual schedule derogation (immediate forced session) is active. |
| `DOK` / `DNOK` | `D:1` / `D:0` | **Toggle Derogation**: Activates (`DOK`) or deactivates (`DNOK`) temporary derogation. Overrides time bands while preserving breaker load-shedding limits. |
| `Dec?` | `Dec:<min>` | **Deferred Start Delay Query**: Returns remaining delay time in minutes before delayed charge initiates. |
| `Dec:<min>` | `Dec:<min>` | **Set Deferred Start Delay**: Configures deferred countdown timer in minutes (e.g., `Dec:120` for 2 hours delay). |
| `Slp?` | `Slp:<0/1>` | **Sleep Query**: Checks if the station is currently in vehicle sleep mode (`1` if `State:W` / `State:Y`). |
| `SOK` / `SNOK` | `SOK:ok*` / `SNOK:ok*` | **Vehicle Sleep Toggle**: `SOK` commands the station into deep sleep mode (`State:W`/`State:Y`); `SNOK` wakes the station and restarts the CP probing sequence. |
| `FM?` | `FM:<val>` | **Functioning Mode Query**: Returns the primary operational profile mask (1=Direct, 2=Auto-hours/TIC, 4=Planning, 5=Modbus, 6=OCPP). |
| `FM:<val>` | `FM:<val>` | Sets the primary operational mode. |
| `FM2?` | `FM2:<0/1>` | Auxiliary Contact Query: Returns simulated dry contact input state. |
| `FM2:<0/1>` | `FM2:<0/1>` | Simulates external contact state (`1` = Peak hours / charge blocked, `0` = Off-peak hours / charge authorized). |
| `CAU?` | `CAU:<0/1>` | Emergency Stop Setpoint Query: Checks state of the emergency stop interlock (SRAM `0x02F1`, `0x02F2`). |
| `CAUOK` / `CAUNOK` | `CAU:1` / `CAU:0` | Toggles emergency stop safety lockout state. |

---

## 6. Modbus RS485 & Energy Metering

Commands to monitor active energy consumption and configure external RS485 bus parameters (Energy Meters and Dynamic Load Management units).

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `EM?` | `EM:<pulses>` | **Energy Meter Pulse Count**: Returns the cumulative active energy pulse counter recorded from the internal/external energy meter. |
| `ModbusAddr?` | `ModbusAddr:<addr>` | Queries the station's slave address on the Modbus RS485 bus (default: `1`). |
| `ModbusAddr:<addr>` | `ModbusAddr:ok*` | Configures the station's Modbus slave address in EEPROM (`0x02F3`). |
| `ModbusEMA?` | `ModbusEMA:<addr>` | Queries the Modbus address assigned to the external Energy Meter (EM). |
| `ModbusEMA:<addr>` | `ModbusEMA:ok*` | Sets the external Energy Meter Modbus address in EEPROM (`0x0D5E`). |
| `ModbusDLMA?` | `ModbusDLMA:<addr>`| Queries the Modbus address of the Dynamic Load Management (DLM) unit. |
| `ModbusDLMA:<addr>`| `ModbusDLMA:ok*` | Sets the DLM Modbus address in EEPROM (`0x0D47`). |
| `ModbusP?` | `ModbusP:<0/1/2>` | Queries Modbus parity (`0` = None, `1` = Even, `2` = Odd). |
| `ModbusP:<0/1/2>` | `ModbusP:ok*` | Sets Modbus parity in EEPROM (`0x02EE`). |
| `ModbusB?` | `ModbusB:<baud>` | Queries Modbus baud rate code (`1`=9600, `2`=19200, etc.). |
| `ModbusB:<baud>` | `ModbusB:ok*` | Sets Modbus baud rate code in EEPROM (`0x02F0`). |
| `ModbusSB?` | `ModbusSB:<1/2>` | Queries Modbus stop bits count (`1` or `2`). |
| `ModbusSB:<1/2>` | `ModbusSB:ok*` | Sets Modbus stop bits count in EEPROM (`0x0CA9`). |

---

## 7. Access Control & RFID Badges

Commands managing user authentication, RFID reader hardware, and OCPP sessions.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `RFIDE?` | `RFIDE:<0/1>` | Queries whether the RFID badge reader peripheral is enabled. |
| `RFIDEOK` / `RFIDENOK` | `RFIDE:1` / `0` | Enables (`RFIDEOK`) or disables (`RFIDENOK`) the RFID badge reader module. |
| `RFIDUM?` | `RFIDUM:<0/1>` | **RFID User Management Mode**: Checks whether RFID access control is enforced locally or delegated to the supervisor. |
| `RFIDA?` | `RFIDA:<0/1>` | Queries whether the station currently has a badge authorization granted. |
| `RFIDA:<tag>` | `RFIDA:ok*` | Transmits an authorization confirmation for a specific badge UID to the ATmega. |
| `RFIDId:<id>` | `RFIDId:<id>` | Transmits the raw badge UID read by the RFID peripheral to the controller. |
| `RFIDIdLP:<id>` | `RFIDIdLP:ok*` | **RFID Local Pass Enrolment**: Enrols a permanent master/local administrative badge directly into the ATmega memory (bypasses cloud/supervisor authorization). |
| `OCPPPS?` | `OCPPStatus:` | **OCPP Power Status**: Returns the standardized OCPP 1.6 status: `Available`, `Preparing-T2S`, `Preparing-DOM`, `Charging`, `SuspendedEVSE`, `SuspendedEV`, `Faulted`, or `Unavailable`. |
| `OCPPPACOK` / `NOK` | `OCPPPAC:1` / `0` | **OCPP Plug and Charge**: Enables or disables badge-less autonomous session authorization under OCPP supervision. |
| `OCPPCTO:<sec>` | `OCPPCTO:ok*` | **OCPP Connection Timeout**: Configures the connection timeout window in seconds (stored in SRAM `0x021D`–`0x0220`). |

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
> All commands ending with the suffix **`TM` (Trace Mode / Test Mode)** are intended strictly for diagnostic probing and serial line sniffing. When activated (`1`), the ATmega continuously broadcasts raw packet dumps and internal state transitions over `Serial1`. They should be left disabled (`0`) during standard operation to prevent serial buffer saturation.

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `TICTM:<0/1>` | `TICTM:1` / `0` | **TIC Trace Mode**: Dumps raw Linky TIC frames, detected baud rates (1200 / 9600), and checksum evaluation results. |
| `RS485TM:<0/1>` | `RS485TM:1` / `0` | **RS485 Trace Mode**: Dumps raw binary packets and request/response cycles exchanged over the RS485 Modbus bus. |
| `BLETM:<0/1>` | `BLETM:1` / `0` | **Bluetooth Trace Mode**: Dumps raw UART traffic exchanged with the u-blox Bluetooth radio module. |
| `DRYCTCTM:<0/1>` | `DRYCTCTM:1` / `0` | **Dry Contact Trace Mode**: Dumps real-time debounce transitions on the auxiliary dry contact inputs (`D27`/`D28`). |

---

## 10. Factory Configuration & Calibration (`NOM:`)

> [!CAUTION]
> **CRITICAL WARNING — PERSISTENT EEPROM WRITES:**  
> The factory write commands (`Reference:`, `HardwareVersion:`, etc.) perform direct, permanent modifications to internal EEPROM addresses (`0x0000`–`0x002F`).  
> **These parameters are NOT cosmetic labels.** The firmware's internal branching logic directly relies on these values at boot. For instance:
> * Changing `HardwareVersion` below `V01.02.00` causes the firmware to route TIC Linky reception to a bit-banged `SoftwareSerial` driver on pin `D10` instead of `HardwareSerial3` (`D14`/`D15`), completely breaking TIC communication on modern boards.
> * Modifying `Reference` alters phase allocation (single-phase vs three-phase) and maximum permissible current ratings.
> * Modifying `coefPh1..3` distorts raw ADC current calibration factors, which can lead to false overcurrent trips (`E:0010`) or unmetered overloading.
> 
> **Never modify these parameters unless replacing a corrupted microcontroller or executing factory recalibration.**

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `Reference:<ref>` | `Reference:ok` | Writes the 6-digit product reference catalogue code to EEPROM `0x0000`–`0x0005`. |
| `HardwareVersion:<hw>`| `HardwareVersion:ok`| Writes the 6-digit hardware revision code (`Vxx.yy.zz`) to EEPROM `0x0006`–`0x000B`. |
| `WeekYearProduction:<wy>`| `WeekYearProduction:ok`| Writes the 6-digit manufacturing date code (`Wwwyy`) to EEPROM `0x000C`–`0x0011`. |
| `SerialNumber:<sn>` | `SerialNumber:ok` | Writes the 6-digit serial number to EEPROM `0x0012`–`0x0017`. |
| `Side:<1/2>` | `Side:ok` | Configures the socket side identifier (`1` or `2`) in EEPROM `0x0018`. |
| `coefPh1:<val>` | `coefPh1:ok` | Calibrates Phase 1 current ADC multiplier factor in EEPROM (`0x0020`). |
| `coefPh2:<val>` | `coefPh2:ok` | Calibrates Phase 2 current ADC multiplier factor in EEPROM (`0x0022`). |
| `coefPh3:<val>` | `coefPh3:ok` | Calibrates Phase 3 current ADC multiplier factor in EEPROM (`0x0024`). |
| `UpdateAuth:<signature>` | `UpdateAuth:ok*` | **Firmware Update Authentication**: Validates a cryptographic signature (computed over Reference, SW version, HW version, Serial, and Week/Year) to arm `g_update_auth_flag = 1`. Required before triggering an emergency Watchdog bootloader jump via byte `0x1B` (ESC). |

---

## 11. Production & Service Test Modes (`service`, `State:Z`)

> [!WARNING]
> **HIGH RISK — UNTESTED & POTENTIALLY UNSAFE:**  
> The Production Mode (`prodModeOK` $\rightarrow$ `State:Z`) and `service` menu are dedicated exclusively to automated factory end-of-line testing benches.  
> **Entering State `'Z'` bypasses all normative IEC 61851-1 safety interlocks.** It grants raw, low-level control to energize 32A power contactors, trigger the 230V shunt trip coil, cycle the motorized socket lock, and disable the watchdog timer.  
> **Executing these commands while a vehicle or human is connected to the station presents severe shock and equipment hazards.**

| Command (TX) | Expected Reply Prefix (RX) | Explanation |
| :--- | :--- | :--- |
| `service` | `---service menu---` | Launches the interactive text diagnostic menu on `Serial1`. Disables watchdog and D3 interrupt. |
| `prodModeOK` | `prodMode:OK*` | **Enters State 'Z' (Factory Test Bench)**: Arms `g_prod_mode_active = 1` and switches the FSM to State `'Z'`, enabling raw component actuators. |
| `prodModeNOK` | `prodMode:NOK*` | **Exits State 'Z'**: Re-arms standard safety watchdogs and returns the FSM to State `'A'`. |
| `testled` | `test:ok` | *(State 'Z' only)* Cycles through Red, Green, and Blue LED rings and front icons. |
| `testbouton` | `test:ok` | *(State 'Z' only)* Tests front panel capacitive touch sensor response. |
| `testcontacteurdomplug` | `test:ok` | *(State 'Z' only)* Manually energizes the auxiliary domestic socket relay (`D31`). |
| `testcontacteurevplug` | `test:ok` | *(State 'Z' only)* **DANGER**: Manually energizes the 32A main power contactors (`D34`/`D35`). |
| `testbobineaemission` | `test:ok` | *(State 'Z' only)* **DANGER**: Pulses the 230V Shunt Trip coil (`D36`) to trip the upstream circuit breaker. |
| `testmoteur` | `test:ok` | *(State 'Z' only)* Cycles the motorized socket lock mechanism back and forth (`D44`..`D46`). |
| `testalimentation` | `test:ok` | *(State 'Z' only)* Verifies 230V AC mains presence sensor on analog pin `A1`. |
| `testCP:<val>` | `test:ok` | *(State 'Z' only)* Calibrates CP ADC voltage thresholds (`12V`, `9V`, `6V`, `3V`, `0V`). |
| `testPP:<val>` | `test:ok` | *(State 'Z' only)* Validates PP resistor thresholds (`13A`, `20A`, `32A`, `63A`). |
| `test6mA` | `test:ok` | *(State 'Z' only)* Triggers self-test on the internal 6mA DC residual current sensor. |

---

## 12. Spontaneous Telemetry (Emitted Unsolicited by ATmega)

The ATmega2560 continuously broadcasts asynchronous notifications on `TX1` regardless of supervisor handshake status:

| Frame (RX) | Transmission Trigger | Explanation |
| :--- | :--- | :--- |
| `RaspberryPi?` | System Boot | Broadcast periodically at power-on to probe for supervisor readiness. |
| `State:<A..Z>` | State Transition | Emitted immediately whenever the charging state machine transitions (e.g., `State:B`, `State:C`). |
| `CCI:<amps>` | During Active Charge | Broadcast periodically while charging to report actual RMS current drawn across the active phases. |
| `E:<0000..0015>` | Error Event / Boot | Emitted upon boot (`E:0000`) and whenever a hardware fault occurs or clears. |
| `SBF:1` / `SBF:0` | Physical Plug Event | `SBF:1` is emitted when a vehicle connects; `SBF:0` is emitted when the cable is unplugged. |
| `R:ok*` / `R:nok*` | Contactor Actuation | Confirms physical auxiliary feedback when the 32A power contactors close or open. |
| `CPh:Mono` / `CPh:Tri` | Charge Startup | Reports detected electrical installation topology (single-phase vs three-phase). |
| `\WT:...:CT:...:\` | Charge Session End | **Session Ticket**: Summarizes Waiting Time (`WT`), Charging Time (`CT`), receptacle used (`EVplug` or `DOMplug`), and accumulated energy during peak/off-peak hours. |
