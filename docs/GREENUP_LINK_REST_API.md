# Green'Up Link - REST API Documentation

This API allows interfacing the Green'Up Link software with any standard home automation controller (Home Assistant, Jeedom, Node-RED, etc.). All responses are in JSON format.

## 🔒 Authentication (Optional)

By default, `api_token` is empty (`""`) in `greenup.toml` (open access on trusted local networks).
If you configure a non-empty `api_token` in `/opt/greenup-link/greenup.toml` (or via the `GREENUP_API_TOKEN` environment variable), all `/api/*` endpoints require authentication by default (`kiosk_mode = false`).
If `kiosk_mode = true` is configured under `[http]`, the Web IHM and read-only `GET` endpoints are accessible without a token, while all write/action endpoints (`POST`, `DELETE`) and `/api/logs/export` remain protected by `api_token`.

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
   curl -u :your_api_token http://<station_ip>:8080/api/status
   ```

*(Note: For browser Server-Sent Events `/api/logs/stream` and file downloads `/api/logs/export`, passing `?token=<your_api_token>` in the query string is also supported).*

---

## 🛡️ Security & Resource Limits

1. **Request Body Size Limit (4 KB)**:
   All incoming HTTP request bodies are globally capped at **4096 bytes (4 KB)** via Axum's `DefaultBodyLimit`. Any request exceeding this limit is immediately rejected with HTTP `413 Payload Too Large` to protect Raspberry Pi system resources.
2. **Raw Command Sanitization**:
   `POST /api/command` strictly validates input against `^[a-zA-Z0-9:?]{1,20}$`, blocking CRLF injection (`\r`, `\n`), escape sequences, and oversized frames on the ATmega serial bus.
3. **Daily Log Size Warning**:
   To safeguard Raspberry Pi microSD endurance, the logger monitors active daily log file growth. If a log file exceeds **10 MB** (and each subsequent 10 MB step, typically when `DEBUG` or `TRACE` is active), a `SYS` warning is emitted to alert the administrator. Logging continues without truncation or interruption so that complete diagnostic data is preserved.
4. **Defensive Security Headers**:
   All HTTP responses served by Green'Up Link include defensive security headers:
   - `X-Frame-Options: DENY`: Prevents embedding the Web UI inside external `<iframe>` elements (clickjacking defense).
   - `X-Content-Type-Options: nosniff`: Prevents MIME-type confusion / sniffing.
   - `Referrer-Policy: no-referrer`: Prevents local station IP and URL paths from leaking in external requests.

---

## 📋 Endpoints Overview

### Canonical & Standard Endpoints (Recommended for EVCC, HA, OCPP)
| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/api/state` | Full canonical station snapshot (Station metadata + Connectors status/meter/control/grid) |
| `GET` | `/api/station` | Station static identity & capabilities (aligned with OCPP `BootNotification`) |
| `GET` | `/api/connectors` | List of all connectors / EVSEs with status, control, meter, grid |
| `GET` | `/api/connectors/:id` | Connector details by index (`1` or `2`) |
| `GET` | `/api/connectors/:id/status` | Connector status (IEC 61851 & OCPP 1.6-J ChargePointStatus) |
| `GET` | `/api/connectors/:id/meter` | Connector electrical metering (Power, Energy, Voltage, Current, Frequency) |
| `GET` | `/api/status` | Real-time status for primary/queried side (`iec`, `ocpp`, `connected`, `charging`) |
| `GET` | `/api/meter` | Real-time electrical metering for primary/queried side (`power_w`, `energy_kwh`, etc.) |

### Authentication
| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/api/auth/status` | Check if authentication is enabled and if the provided token is valid |

### Control & Actions
| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `POST` | `/api/charge` | Control charge state (`enable`, `disable`, `pause`, `resume`) |
| `POST` | `/api/current/:amps` | Current limit (`CCEl:XX`, 7A to 32A) |
| `POST` | `/api/voltage` | Inject external grid voltage (`{"voltage": 230.0}`) |
| `POST` | `/api/init` | Relaunch startup sequence |
| `POST` | `/api/reset` | ATmega board hardware restart (`Reset`) |
| `POST` | `/api/tic/refresh` | TIC detection (only in `State:A`) |
| `POST` | `/api/bluetooth` | Enable / disable Bluetooth |
| `POST` | `/api/command` | Raw serial command (debug, strictly guarded) |

### Logs & Diagnostics
| Method | Endpoint | Description |
| :--- | :--- | :--- |
| `GET` | `/api/logs` | Recent in-memory logs (last 500 entries) |
| `GET` | `/api/logs/stream` | Real-time Server-Sent Events (SSE) log stream |
| `GET` | `/api/logs/export` | Download the current daily log file (`.log`) |
| `GET` | `/api/logs/level` | Get currently active log level |
| `POST` | `/api/logs/level` | Update active log level dynamically at runtime |

---

## 📡 Read Endpoints (GET)

### 1. Get canonical station snapshot (OCPP 1.6-J & EVCC structured)
- **URL**: `/api/state`
- **Method**: `GET`
- **Description**: Returns the unified, clean hierarchical data model for the entire charging station, including station-level metadata and an array of individual connectors (`EVSEs`). Designed as the canonical backbone for OCPP 1.6-J, EVCC, and Home Assistant.
- **Typical Response**:
  ```json
  {
    "station": {
      "charge_point_vendor": "Legrand",
      "charge_point_model": "Green'Up Premium (058001)",
      "charge_point_serial_number": "015184",
      "firmware_version": "V01.18.10",
      "hardware_version": "V02.01.01",
      "production_date": "42W22",
      "link_version": "0.5.0-dev",
      "connector_count": 1,
      "capabilities": {
        "phases": 1,
        "max_power_kw": 7.4,
        "max_current_amps": 32,
        "has_schuko": false,
        "charging_points": 1,
        "is_known": true,
        "has_tic": true
      },
      "serial": {
        "status": "usb_connected",
        "port": "/dev/ttyUSB0",
        "sniffer_mode": false,
        "message": "USB serial link active - Station connected",
        "last_rx_secs_ago": 0
      }
    },
    "connectors": [
      {
        "connector_id": 1,
        "status": {
          "iec": "C",
          "ocpp": "Charging",
          "legrand_raw": "C",
          "connected": true,
          "charging": true,
          "error_code": "0000",
          "error_description": "No error"
        },
        "control": {
          "setpoint_amps": 16,
          "applied_limit_amps": 16,
          "pilot_limit_amps": 16,
          "min_allowed_amps": 7,
          "max_hardware_amps": 32,
          "charge_authorized": true,
          "pilot_voltage": 6
        },
        "meter": {
          "power_w": 3680.0,
          "energy_kwh": 14.52,
          "current_a": 16.0,
          "voltage_v": 230.0,
          "frequency_hz": 50.0,
          "voltage_source": "internal"
        },
        "grid": {
          "tic_mode": "9600",
          "tic_active": true
        }
      }
    ]
  }
  ```

### 2. Get station identity & capabilities
- **URL**: `/api/station`
- **Method**: `GET`
- **Description**: Returns only the station-level metadata (vendor, model, serial, firmware, hardware capabilities, serial connection status). Formatted directly to populate an OCPP `BootNotification` payload.
- **Typical Response**:
  ```json
  {
    "charge_point_vendor": "Legrand",
    "charge_point_model": "Green'Up Premium (058001)",
    "charge_point_serial_number": "015184",
    "firmware_version": "V01.18.10",
    "hardware_version": "V02.01.01",
    "production_date": "42W22",
    "link_version": "0.5.0-dev",
    "connector_count": 1,
    "capabilities": { ... },
    "serial": {
      "status": "usb_connected",
      "port": "/dev/ttyUSB0",
      "sniffer_mode": false,
      "message": "USB serial link active - Station connected",
      "last_rx_secs_ago": 0
    }
  }
  ```

### 3. Get connector list & specific connector data
- **URLs**:
  - `GET /api/connectors`: List all connectors of the station (`Vec<ConnectorData>`).
  - `GET /api/connectors/:id`: Get full data for connector `:id` (`1` or `2`).
  - `GET /api/connectors/:id/status`: Get only `status` for connector `:id`.
  - `GET /api/connectors/:id/meter`: Get only electrical `meter` for connector `:id`.
- **Typical Response (`GET /api/connectors/1/status`)**:
  ```json
  {
    "iec": "C",
    "ocpp": "Charging",
    "legrand_raw": "C",
    "connected": true,
    "charging": true,
    "error_code": "0000",
    "error_description": "No error"
  }
  ```
- **Typical Response (`GET /api/connectors/1/meter`)**:
  ```json
  {
    "power_w": 3680.0,
    "energy_kwh": 14.52,
    "current_a": 16.0,
    "voltage_v": 230.0,
    "frequency_hz": 50.0,
    "voltage_source": "internal"
  }
  ```

### 4. Fast Status & Metering Endpoints (Optimized for EVCC & Home Assistant)
- **URLs**:
  - `GET /api/status[?side=1|2]`
  - `GET /api/meter[?side=1|2]`
- **Description**: Direct, lightweight endpoints targeting the primary socket (or queried `?side=`). Perfect for simple HTTP polls in EVCC or Home Assistant REST sensors without parsing a giant JSON blob.
- **`GET /api/status` Response**:
  ```json
  {
    "iec": "C",
    "ocpp": "Charging",
    "legrand_raw": "C",
    "connected": true,
    "charging": true,
    "error_code": "0000",
    "error_description": "No error"
  }
  ```
- **`GET /api/meter` Response**:
  ```json
  {
    "power_w": 3680.0,
    "energy_kwh": 14.52,
    "current_a": 16.0,
    "voltage_v": 230.0,
    "frequency_hz": 50.0,
    "voltage_source": "internal"
  }
  ```

---

## 🛠️ Action Endpoints (POST)

> [!IMPORTANT]
> **USB Connection Guard:** If the USB serial link with the station is not active (`serial.status != "usb_connected"`), all POST action endpoints immediately return HTTP `503 Service Unavailable` with a descriptive message:
> ```json
> {
>   "status": "error",
>   "error": "Command unavailable: USB cable disconnected (no USB adapter detected)",
>   "serial_status": "usb_disconnected"
> }
> ```

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
- **Description**: Sets the current limit target. When the station is in `State:A` (vehicle unplugged), Green'Up Link sends both `CCEl:XX` and `CC:XX` to keep the ATmega limits synchronized while idle. In all other operating states (`B`, `C`, etc.), only `CCEl:XX` is sent, allowing the ATmega to dynamically consolidate internal hardware limits (`CC = min(DIP, CCCa, CCEl, CCTIC)`).
- **Security**: The API rejects any value outside `[7, 32]` with a `400` error. While the ATmega board can technically accept values above 32A, they have not been tested in real conditions to avoid hardware risks. Using `CCEl` (Eliot limit) instead of directly forcing `CC` in active states is a deliberate safety choice: it allows the ATmega to keep its internal safeguards active, computing the final safe limit (`CC`) based on other hardware factors. Finally, note that below 6A the station crashes (`E:0010`), and the electronics do not regulate below ~6.8A.
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
- **Validation & Security**:
  - Whitelist: only characters matching `[a-zA-Z0-9:?]` are accepted (strictly eliminates CRLF `\r`, `\n`, control characters, and injection attempts).
  - Length: maximum 20 characters (`1..=20`).
  - Blocked in Sniffer mode (`sniffer_mode = true` → `403 Forbidden`).
  - If invalid, returns HTTP `400 Bad Request`:
    ```json
    {
      "status": "error",
      "error": "Invalid command format. Allowed characters: [a-zA-Z0-9:?], length 1-20 characters."
    }
    ```
- **Typical Response**:
  ```json
  { "status": "success", "message": "Command sent" }
  ```

### 10. Inject external grid voltage
- **URL**: `/api/voltage`
- **Method**: `POST`
- **Body (JSON)**:
  ```json
  { "voltage": 234.5 }
  ```
- **Description**: Injects a measured grid voltage from an external smart meter (Shelly, Enedis Linky, Home Assistant, etc.). When injected, Green'Up Link uses this exact voltage to compute active power ($P = U \times I$) instead of the default nominal 230V. External voltage automatically expires after 30 seconds if not refreshed.
- **Validation**: Accepts voltage values between `180.0` and `280.0` V. Allowed even in Sniffer mode (`sniffer_mode = true`).
- **Typical Response**:
  ```json
  { "status": "success", "message": "External voltage updated" }
  ```

---

## 📋 Log Management Endpoints

### 12. Get recent in-memory logs
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
      "message": "🔍 GET request /api/state"
    }
  ]
  ```

### 13. Stream real-time logs (SSE)
- **URL**: `/api/logs/stream`
- **Method**: `GET`
- **Content-Type**: `text/event-stream`
- **Description**: Server-Sent Events (SSE) stream broadcasting all newly generated log entries in real-time. Automatic keep-alive pings are transmitted to keep persistent connections alive.

### 14. Download daily log file
- **URL**: `/api/logs/export`
- **Method**: `GET`
- **Content-Type**: `text/plain; charset=utf-8`
- **Content-Disposition**: `attachment; filename="greenup_YYYY-MM-DD.log"`
- **Description**: Downloads the raw text content of the active daily log file on the server.

### 15. Get active log level
- **URL**: `/api/logs/level`
- **Method**: `GET`
- **Description**: Returns the currently active logging threshold filter (applied equally to stdout console and disk files).
- **Typical Response**:
  ```json
  {
    "level": "INFO"
  }
  ```

### 16. Update active log level dynamically
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