# Green'Up Link - REST API Documentation

This API allows interfacing the Green'Up Link software with any standard home automation controller (Home Assistant, Jeedom, Node-RED, etc.). All responses are in JSON format.

## 📋 Endpoints Overview

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/api/info` | Hardware info (Firmware, Serial, Capabilities) |
| `GET` | `/api/telemetry` | Real-time telemetry + Legrand / IEC states |
| `POST` | `/api/charge/enable` | Force charge (`DOK`, bypasses restrictions) |
| `POST` | `/api/charge/disable` | Secure stop (`FM2:1 + DNOK`) |
| `POST` | `/api/charge/pause` | Natural pause (`SBNOK`) |
| `POST` | `/api/charge/resume` | Natural resume (`SBOK`) |
| `POST` | `/api/current/:amps` | Current limit (`CCEl:XX`) |
| `POST` | `/api/init` | Relaunch startup sequence |
| `POST` | `/api/reset` | ATmega board hardware restart (`Reset`) |
| `POST` | `/api/tic/refresh` | TIC detection (only in `State:A`) |
| `POST` | `/api/bluetooth` | Enable / disable Bluetooth |
| `POST` | `/api/command` | Raw serial command (debug) |

---

## 📡 Read Endpoints (GET)

### 1. Get hardware and software information
- **URL**: `/api/info`
- **Method**: `GET`
- **Description**: Returns power board identification info (firmware version, serial number, production date...) as well as hardware capabilities deduced from the reference (`capabilities`). This info is queried at startup and cached.
- **Typical Response**:
  ```json
  {
    "software_version": "V01.18.10",
    "hardware_version": "V02.01.01",
    "serial_number": "015184",

    "week_year_production": "42W22",
    "bluetooth_enabled": false,
    "link_version": "0.1.0",
    "capabilities": {


      "phases": 1,
      "max_power_kw": 7.4,
      "max_current_amps": 32,
      "has_schuko": false,
      "charging_points": 1,
      "is_known": true
    }
  }
  ```
  - *Note:* `capabilities` is `null` until the reference has been received.

### 2. Get real-time telemetry
- **URL**: `/api/telemetry`
- **Method**: `GET`
- **Description**: Returns real-time station data. Designed to be polled frequently (polling ~2s by the GUI). Calls are logged at `TRACE` level (log file only, not console).
- **Typical Response**:
  ```json
  {
    "voltage": 232.0,
    "current": 15.6,
    "power": 3619.2,
    "energy": 12.5,
    "frequency": 50.0,
    "error_code": "0000",
    "limit_amps": 16,
    "eliot_limit_amps": 16,
    "cp_voltage": 6,


    "iec_state": "Charging_C",
    "charge_complete": false,
    "tic_mode": "9600"
  }
  ```
  - `greenup_state`: Legrand proprietary state (`A`, `B`, `C`, `D`, `E`, `I`, `W`, `M`, `L`, `R`, `X`, `V`). See `docs/STATE_MACHINE.md`.
  - `iec_state`: Deduced standard IEC state: `Disconnected_A`, `Connected_B`, `Charging_C`, `Error_E`, `Faulted_F`. Returns `null` if it cannot be deduced.
  - `cp_voltage` (`CP?`): Control Pilot voltage in volts (`12`, `9`, `6`).

  - `eliot_limit_amps` (`CCEl?`): Effective current limit applied by the station.
  - `tic_mode`: `"0"` (absent), `"1200"` (historical), `"9600"` (standard), `"detecting"` (detection in progress) or `""` (unknown, no detection started).

---

## 🛠️ Action Endpoints (POST)

### 3. Enable charge
- **URL**: `/api/charge/enable`
- **Method**: `POST`
- **Description**: Authorizes and forces the charge. First sends `T2COK` to ensure the Type 2 port is active, then `FM2:0` to unblock any external restrictions, then activates Derogation (`DOK`). This overrides the physical TIC (Peak/Off-Peak) and the external signal (`FM2`) limitations, while keeping the local hardware load-balancing (Délestage) active to protect the main breaker. The station will report `State:D` (Derogation Charge) which is equivalent to `State:C`.
- **Typical Response**:
  ```json
  { "status": "success", "message": "Charge enabled (T2COK + FM2:0 + DOK)" }
  ```

### 4. Disable charge
- **URL**: `/api/charge/disable`
- **Method**: `POST`
- **Description**: Securely disables the charge. First sends `FM2:1` to firmly block the charge via the simulated external contact (preventing TIC from accidentally starting it during Off-Peak hours), then sends `DNOK` to release the derogation.
- **Typical Response**:
  ```json
  { "status": "success", "message": "Charge disabled (FM2:1 + DNOK)" }
  ```

### 5. Pause charge (Natural)
- **URL**: `/api/charge/pause`
- **Method**: `POST`
- **Description**: Pauses the charge naturally by simulating a press on the physical Stop button (`SBNOK`). This puts the station in a paused state (`SB:1`).
- **Typical Response**:
  ```json
  { "status": "success", "message": "Charge paused (SBNOK)" }
  ```

### 6. Resume charge (Natural)
- **URL**: `/api/charge/resume`
- **Method**: `POST`
- **Description**: Resumes the charge naturally by simulating a press on the physical Start button (`SBOK`), removing the paused state (`SB:0`).
- **Typical Response**:
  ```json
  { "status": "success", "message": "Charge resumed (SBOK)" }
  ```

### 7. Modify current limit
- **URL**: `/api/current/:amps`
- **Method**: `POST`
- **URL Parameter**: `:amps` (integer between **7** and **32**).
- **Description**: Sets the current limit target (sends `CCEl:XX`, always 2-digit formatted).
- **Security**: The API rejects any value outside `[7, 32]` with a `400` error. While the ATmega board can technically accept values above 32A, they have not been tested in real conditions to avoid hardware risks. Using `CCEl` (Eliot limit) instead of directly forcing `CC` is a deliberate choice: it allows the ATmega to keep its internal safeguards active, computing the final safe limit (`CC`) based on other hardware factors. Finally, note that below 6A the station crashes (`E:0010`), and the electronics do not regulate below ~6.8A.
- **Typical Response**:
  ```json
  { "status": "success", "message": "Current limit set to 16A" }
  ```

### 8. Relaunch initialization sequence
- **URL**: `/api/init`
- **Method**: `POST`
- **Description**: Replays the startup sequence: `RaspberryPiModeOK`, then querying information (`SoftwareVersion?`, `HardwareVersion?`, `SerialNumber?`, `Reference?`, `WeekYearProduction?`) and states (`State?`, `FM?`, `CC?`, `CCEl?`, `CP?`, `T2C?`, `SB?`, `E?`, `BT?`). The response is immediate, the sequence runs in the background.
- **Note**: TIC detection is **never** started automatically. It must be requested explicitly via `/api/tic/refresh`.
- **Typical Response**:
  ```json
  { "status": "success", "message": "Initialization sequence started" }
  ```

### 9. Restart power board
- **URL**: `/api/reset`
- **Method**: `POST`
- **Description**: Sends the `Reset` command to the ATmega board to force a hardware restart. Useful to unblock the station when it remains stuck in `State:A` while `T2C:1` and `CP:9` (vehicle plugged in): state `B` never arrives without a reset.
- **Typical Response**:
  ```json
  { "status": "success", "message": "ATmega board restart requested" }
  ```

### 10. Start TIC (Linky) detection
- **URL**: `/api/tic/refresh`
- **Method**: `POST`
- **Description**: Starts the customer tele-information (TIC) speed detection. The server sends `TICTM:1`, parses the return (e.g. `TICTestB:9600`), updates `tic_mode`, then disables test mode with `TICTM:0`.
- **Security**: Detection is only allowed if the station is idle (`greenup_state == "A"`). In any other state, **nothing is sent to the station** and the API responds `400`:
  ```json
  { "error": "TIC detection can only be done when the station is free (State:A). Current state: M" }
  ```
- **Typical Response (Success)**:
  ```json
  { "status": "success", "message": "TIC detection started" }
  ```

### 11. Enable / Disable Bluetooth
- **URL**: `/api/bluetooth`
- **Method**: `POST`
- **Body (JSON)**:
  ```json
  { "enabled": true }
  ```
- **Description**: Turns the station's Bluetooth module on (`enabled: true`) or off (`enabled: false`) (`BTOK` / `BTNOK`). Useful to prevent the Legrand smartphone app from overriding home automation instructions.
- **Typical Response**:
  ```json
  { "status": "success", "message": "Bluetooth disabled successfully" }
  ```

### 12. Send raw command (Debug/Advanced)
- **URL**: `/api/command`
- **Method**: `POST`
- **Body (Plain text)**: the command to send (e.g., `State?`)
- **Description**: Directly transmits the string on the serial port. Reserved for testing and debugging. See [LEGRAND_SERIAL_PROTOCOL.md](LEGRAND_SERIAL_PROTOCOL.md) for the list of known commands.
- **Typical Response**:
  ```json
  { "status": "success", "message": "Command sent" }
  ```


