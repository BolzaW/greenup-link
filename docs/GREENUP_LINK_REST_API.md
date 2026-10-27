# Green'Up Link - REST API Documentation

This API allows interfacing the Green'Up Link software with any standard home automation controller (Home Assistant, Jeedom, Node-RED, etc.). All responses are in JSON format.

## 🔒 Authentication (Optional)

By default, `api_token` is empty (`""`) in `greenup.toml` (open access on trusted local networks).
If you configure a non-empty `api_token` in `/opt/greenup-link/greenup.toml` (or via the `GREENUP_API_TOKEN` environment variable), all `/api/*` endpoints require authentication.

Three standard authentication methods are supported simultaneously against the same `api_token`:

1. **Bearer Token (Recommended for EVCC, Home Assistant, scripts)**:
   ```http
   Authorization: Bearer <your_api_token>
   ```
2. **API Key Header**:
   ```http
   X-API-Key: <your_api_token>
   ```
3. **HTTP Basic Auth** (with any username or empty username, and `<your_api_token>` as password):
   ```bash
   curl -u :your_api_token http://<station_ip>:8080/api/telemetry
   ```

*(Note: For browser Server-Sent Events `/api/logs/stream` and file downloads `/api/logs/export`, passing `?token=<your_api_token>` in the query string is also supported).*

---

## 📋 Endpoints Overview

| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/api/auth/status` | Check if authentication is enabled and if the provided token is valid |
| `GET` | `/api/info` | Hardware info (Firmware, Serial, Capabilities) |
| `GET` | `/api/telemetry` | Real-time telemetry + Legrand / IEC states |
| `GET` | `/api/logs` | Recent in-memory logs (last 500 entries) |
| `GET` | `/api/logs/stream` | Real-time Server-Sent Events (SSE) log stream |
| `GET` | `/api/logs/export` | Download the current daily log file (`.log`) |
| `GET` | `/api/logs/level` | Get currently active log level |
| `POST` | `/api/logs/level` | Update active log level dynamically at runtime |
| `POST` | `/api/charge` | Control charge state (`enable`, `disable`, `pause`, `resume`) |
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
    "reference": "058001",
    "week_year_production": "42W22",
    "bluetooth_enabled": false,
    "mac_address": "000000000000",
    "side": 1,
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
  - `side`: Active socket side (`1` or `2`), decoded from the ATmega `Side:` response (`49` -> `1`, `50` -> `2`).
  - `mac_address`: Bluetooth controller MAC address (12-char hex or `"000000000000"` if BLE is disabled).
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
    "charge_authorized": true,
    "charge_paused": false,
    "greenup_state": "C",
    "iec_state": "C",
    "charge_complete": false,
    "tic_mode": "9600"
  }
  ```
  - `greenup_state`: Legrand proprietary state (`A`, `B`, `C`, `D`, `E`, `I`, `W`, `M`, `L`, `R`, `X`, `V`). See [`STATE_MACHINE.md`](STATE_MACHINE.md).
  - `iec_state`: Deduced standard IEC 61851-1 state (`A`, `B`, `C`, `E`, `F`). Returns `null` if it cannot be deduced.
  - `cp_voltage` (`CP?`): Control Pilot voltage in volts (`12`, `9`, `6`).
  - `charge_authorized`: Indicates whether charging is currently authorized (`true`) or blocked (`false`).
  - `charge_paused`: Indicates whether charging has been paused via the Stop button (`true`).
  - `eliot_limit_amps` (`CCEl?`): Effective current limit applied by the station.
  - `charge_complete`: True if charging has terminated (vehicle battery full).
  - `tic_mode`: `"0"` (absent), `"1200"` (historical), `"9600"` (standard), `"detecting"` (detection in progress) or `""` (unknown, no detection started).

---

## 🛠️ Action Endpoints (POST)

### 3. Control charge state
- **URL**: `/api/charge`
- **Method**: `POST`
- **Body (JSON)**:
  ```json
  { "action": "enable" }
  ```
  *(Supported actions: `"enable"`, `"disable"`, `"pause"`, `"resume"`)*

#### Actions detail:
- **`enable`**: Authorizes and forces the charge. First sends `T2COK` to ensure the Type 2 port is active, then `FM2:0` to unblock any external restrictions, then activates Derogation (`DOK`). This overrides the physical TIC (Peak/Off-Peak) and the external signal (`FM2`) limitations, while keeping the local hardware load-balancing (Délestage) active to protect the main breaker. The station will report `State:D` (Derogation Charge) which is equivalent to `State:C`.
- **`disable`**: Securely disables the charge. First sends `FM2:1` to firmly block the charge via the simulated external contact (preventing TIC from accidentally starting it during Off-Peak hours), then sends `DNOK` to release the derogation.
- **`pause`**: Pauses the charge naturally by simulating a press on the physical Stop button (`SBNOK`). This puts the station in a paused state (`SB:1`).
- **`resume`**: Resumes the charge naturally by simulating a press on the physical Start button (`SBOK`), removing the paused state (`SB:0`).

- **Typical Response**:
  ```json
  { "status": "success" }
  ```

### 4. Modify current limit
- **URL**: `/api/current/:amps`
- **Method**: `POST`
- **URL Parameter**: `:amps` (integer between **7** and **32**).
- **Description**: Sets the current limit target (sends `CCEl:XX`, always 2-digit formatted).
- **Security**: The API rejects any value outside `[7, 32]` with a `400` error. While the ATmega board can technically accept values above 32A, they have not been tested in real conditions to avoid hardware risks. Using `CCEl` (Eliot limit) instead of directly forcing `CC` is a deliberate choice: it allows the ATmega to keep its internal safeguards active, computing the final safe limit (`CC`) based on other hardware factors. Finally, note that below 6A the station crashes (`E:0010`), and the electronics do not regulate below ~6.8A.
- **Typical Response**:
  ```json
  { "status": "success", "message": "Current limit set to 16A" }
  ```

### 5. Relaunch initialization sequence
- **URL**: `/api/init`
- **Method**: `POST`
- **Description**: Replays the startup sequence: `RaspberryPiModeOK`, then querying information (`SoftwareVersion?`, `HardwareVersion?`, `SerialNumber?`, `Reference?`, `WeekYearProduction?`) and states (`State?`, `FM?`, `CC?`, `CCEl?`, `CP?`, `T2C?`, `SB?`, `E?`, `BT?`). The response is immediate, the sequence runs in the background.
- **Note**: TIC detection is **never** started automatically. It must be requested explicitly via `/api/tic/refresh`.
- **Typical Response**:
  ```json
  { "status": "success", "message": "Initialization sequence started" }
  ```

### 6. Restart power board
- **URL**: `/api/reset`
- **Method**: `POST`
- **Description**: Sends the `Reset` command to the ATmega board to force a hardware restart. Useful to unblock the station when it remains stuck in `State:A` while `T2C:1` and `CP:9` (vehicle plugged in): state `B` never arrives without a reset.
- **Typical Response**:
  ```json
  { "status": "success", "message": "ATmega board restart requested" }
  ```

### 7. Start TIC (Linky) detection
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

### 8. Enable / Disable Bluetooth
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

### 9. Send raw command (Debug/Advanced)
- **URL**: `/api/command`
- **Method**: `POST`
- **Body (Plain text)**: the command to send (e.g., `State?`)
- **Description**: Directly transmits the string on the serial port. Reserved for testing and debugging. See [`LEGRAND_SERIAL_PROTOCOL.md`](LEGRAND_SERIAL_PROTOCOL.md) for the list of known commands.
- **Typical Response**:
  ```json
  { "status": "success", "message": "Command sent" }
  ```

---

## 📋 Log Management Endpoints

### 10. Get recent in-memory logs
- **URL**: `/api/logs`
- **Method**: `GET`
- **Description**: Returns the ring buffer of the most recent 500 log entries recorded by the server since startup. Used by the Web UI console to populate the history on page load.
- **Typical Response**:
  ```json
  [
    {
      "timestamp": "2026-10-16 00:15:20.123",
      "category": "SERIAL_RX",
      "message": "State:A"
    },
    {
      "timestamp": "2026-10-16 00:15:21.050",
      "category": "API",
      "message": "🔍 GET request /api/telemetry"
    }
  ]
  ```

### 11. Stream real-time logs (SSE)
- **URL**: `/api/logs/stream`
- **Method**: `GET`
- **Content-Type**: `text/event-stream`
- **Description**: Server-Sent Events (SSE) stream broadcasting all newly generated log entries in real-time. Automatic keep-alive pings are transmitted to keep persistent connections alive.

### 12. Download daily log file
- **URL**: `/api/logs/export`
- **Method**: `GET`
- **Content-Type**: `text/plain; charset=utf-8`
- **Content-Disposition**: `attachment; filename="greenup_YYYY-MM-DD.log"`
- **Description**: Downloads the raw text content of the active daily log file on the server.

### 13. Get active log level
- **URL**: `/api/logs/level`
- **Method**: `GET`
- **Description**: Returns the currently active logging threshold filter (applied equally to stdout console and disk files).
- **Typical Response**:
  ```json
  {
    "level": "INFO"
  }
  ```

### 14. Update active log level dynamically
- **URL**: `/api/logs/level`
- **Method**: `POST`
- **Body (JSON)**:
  ```json
  {
    "level": "DEBUG"
  }
  ```
  *(Supported levels: `"ERROR"`, `"WARN"`, `"INFO"`, `"DEBUG"`, `"TRACE"`)*
- **Description**: Dynamically updates the logging threshold filter at runtime without requiring a service or Raspberry Pi restart.
- **Typical Response**:
  ```json
  {
    "status": "success",
    "level": "DEBUG"
  }
  ```
