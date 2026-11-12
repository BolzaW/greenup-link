# Green'up Link MQTT Integration & Home Assistant Auto-Discovery

Green'up Link features a built-in, fully autonomous **MQTT 3.1.1 client** designed to integrate your Legrand Green'up charging station with smart home platforms like **Home Assistant**, **EVCC**, **Node-RED**, and **Jeedom**.

---

## 1. Key Features

- **Autonomous Client**: Connects directly to your local MQTT broker (Mosquitto, Home Assistant MQTT add-on, etc.).
- **Unified Authentication**: Uses the existing `http.api_token` as the MQTT password (with optional configurable `username`).
- **Atomic Unitary Topics (`Report-by-Exception`)**: Every electrical, state, and control metric is published on its own dedicated topic as a raw scalar value (e.g. `greenup/connector/1/meter/active_power_w` -> `7360.0`). Topics are updated instantly on state changes and fully refreshed every 60 seconds with `retain: true`.
- **Home Assistant MQTT Auto-Discovery**: Automatically exposes the charging station and all active connectors to Home Assistant as a clean device with sensors, switches, numbers, and binary sensors. Zero manual YAML configuration required in Home Assistant!
- **Remote Control Topics (`.../set`)**: Start, stop, pause, adjust current setpoints (7–32A), and inject external grid voltage via MQTT.
- **Hardware Protection & Safety**:
  - Respects **Sniffer Mode**: all TX control commands are rejected if passive monitoring is enabled.
  - State A Synchronization: automatically keeps `CCEl` and `CC` synchronized when in `State:A`, and never overrides ATmega minimum current logic in other states.
  - Range validation on current (7–32A) and voltage (100–300V).

---

## 2. Configuration (`greenup.toml`)

MQTT is configured in the `[mqtt]` section of your `greenup.toml`:

```toml
[http]
bind = "0.0.0.0:8080"
# When set, this token also secures your MQTT client connection!
api_token = "my_secure_token_123"
kiosk_mode = false

[mqtt]
# Enable or disable the MQTT client:
# - false : (Default) MQTT client is not started.
# - true  : Green'up Link connects to the MQTT broker, publishes telemetry, and listens for commands.
enabled = true

# MQTT broker host and port (e.g. Mosquitto / Home Assistant broker).
# Default: "127.0.0.1:1883"
broker = "192.168.1.100:1883"

# Optional MQTT username.
# When api_token is set under [http], greenup-link authenticates
# using `username` (defaults to "greenup-link" if empty) and `http.api_token` as password.
# Leave empty for anonymous connection if your broker requires no credentials.
username = "homeassistant"

# Base MQTT topic prefix.
# Default: "greenup"
base_topic = "greenup"

# Publish Home Assistant MQTT Auto-Discovery payloads (`homeassistant/.../config`).
# Default: true
ha_discovery = true
```

---

## 3. MQTT Topic Reference

All published telemetry topics have `retain: true` enabled.

### 3.1 Availability & LWT (Last Will and Testament)

| Topic | Values | Retain | Description |
| :--- | :--- | :--- | :--- |
| `<base>/availability` | `online` / `offline` | Yes | Service availability status. Automatically set to `offline` by broker if connection drops. |

### 3.2 Station Topics (`<base>/station/...`)

| Topic | Example Value | Description |
| :--- | :--- | :--- |
| `<base>/station/model` | `059001` | Legrand commercial reference |
| `<base>/station/serial_number` | `059001A12345` | Unique serial number |
| `<base>/station/firmware_version` | `1.19.02` | ATmega mainboard firmware version |
| `<base>/station/hardware_version` | `V01.00` | ATmega hardware revision |
| `<base>/station/link_version` | `0.5.0` | Green'up Link daemon version |
| `<base>/station/mac_address` | `00:04:74:XX:XX:XX` | Legrand MAC address |
| `<base>/station/is_dual` | `false` / `true` | Dual charge point station indicator |
| `<base>/station/available_connectors` | `1` or `1,2` | Active charge point IDs |
| `<base>/station/serial/status` | `usb_connected` | USB link state (`usb_connected`, `usb_disconnected`, etc.) |
| `<base>/station/serial/port` | `/dev/ttyUSB0` | Active serial device path |
| `<base>/station/serial/sniffer_mode` | `false` / `true` | Sniffer mode (passive monitoring) state |
| `<base>/station/total_active_power_w` | `7360.0` | Total active power across all active connectors (W) |
| `<base>/station/voltage_v` | `232.4` | Effective grid voltage (V) |
| `<base>/station/voltage_source` | `default` / `external` | Voltage source (`default` = 230V, `external` = injected) |

---

### 3.3 Charge Point Topics (`<base>/connector/<id>/...`)

> `<id>` is `1` for the right / default charge point, and `2` for the left charge point on dual-socket stations.

#### Status & State (`.../status/...`)

| Topic | Example Value | Description |
| :--- | :--- | :--- |
| `<base>/connector/<id>/status/state` | `A`, `B`, `C`, `E`, `F` | IEC 61851-1 operating state |
| `<base>/connector/<id>/status/ocpp_status` | `Charging`, `Available`, ... | Standard OCPP 1.6-J ChargePointStatus |
| `<base>/connector/<id>/status/raw_code` | `D`, `A`, `B`, `J`, ... | Raw Legrand internal state machine letter |
| `<base>/connector/<id>/status/vehicle_connected` | `true` / `false` | Vehicle plugged into the socket |
| `<base>/connector/<id>/status/charging` | `true` / `false` | Active power delivery in progress |
| `<base>/connector/<id>/status/paused` | `true` / `false` | Charge paused via software button |
| `<base>/connector/<id>/status/charge_complete` | `true` / `false` | Battery full / charging ended by EV |
| `<base>/connector/<id>/status/error_code` | `0000` | ATmega hardware fault code |

#### Electrical Metering (`.../meter/...`)

| Topic | Unit | Example Value | Description |
| :--- | :--- | :--- | :--- |
| `<base>/connector/<id>/meter/active_power_w` | W | `7360.0` | Real-time charging active power |
| `<base>/connector/<id>/meter/energy_total_kwh` | kWh | `124.582` | Total cumulative energy delivered |
| `<base>/connector/<id>/meter/energy_total_wh` | Wh | `124582.0` | Total cumulative energy delivered |
| `<base>/connector/<id>/meter/voltage_v` | V | `231.8` | Real-time voltage |
| `<base>/connector/<id>/meter/current_amps` | A | `31.85` | Real-time current delivered to the EV |
| `<base>/connector/<id>/meter/frequency_hz` | Hz | `50.0` | Grid frequency |

#### Charging Control & Consignes (`.../control/...`)

| Topic | Unit | Example Value | Description |
| :--- | :--- | :--- | :--- |
| `<base>/connector/<id>/control/charge` | - | `ON` / `OFF` | Master charge authorization switch state |
| `<base>/connector/<id>/control/pause` | - | `true` / `false` | Temporary pause button state |
| `<base>/connector/<id>/control/setpoint_amps` | A | `16` | Current setpoint configured (`CCEl`) |
| `<base>/connector/<id>/control/applied_limit_amps` | A | `16` | Effective current limit applied by ATmega (`CC`) |
| `<base>/connector/<id>/control/min_allowed_amps` | A | `7` | Minimum allowed current (Legrand standard: 7A) |
| `<base>/connector/<id>/control/max_hardware_amps` | A | `32` | Maximum hardware capacity of the station |

#### Linky TIC Grid Status (`.../grid/...`)

| Topic | Example Value | Description |
| :--- | :--- | :--- |
| `<base>/connector/<id>/grid/tic_connected` | `true` / `false` | Linky TIC module connected |
| `<base>/connector/<id>/grid/tic_mode` | `standard`, `historic`, `none` | TIC baud rate mode (9600, 1200, none) |

#### Charging Session History (`.../session/...`)

| Topic | Unit | Example Value | Description |
| :--- | :--- | :--- | :--- |
| `<base>/connector/<id>/session/active` | - | `true` / `false` | Active session currently in progress |
| `<base>/connector/<id>/session/duration_seconds` | s | `3742` | Total duration connected |
| `<base>/connector/<id>/session/energy_wh` | Wh | `14250.0` | Total energy delivered during this session |

---

## 4. Remote Control Command Topics (`.../set`)

Publishing to these topics allows external systems to pilot the charging station.

### 4.1 Master Charge Switch

**Topic**: `<base>/connector/<id>/control/charge/set`

| Payload | Action |
| :--- | :--- |
| `enable`, `ON`, `1`, `true`, `resume` | Authorizes charging (`AuthorizeType2(true) + SetDerogation(true)`) |
| `disable`, `OFF`, `0`, `false` | Disables charging (`SetExternalSignal(true) + SetDerogation(false)`) |
| `pause` | Pauses charging (`SetStartButton(false)`) |

### 4.2 Pause / Resume Switch

**Topic**: `<base>/connector/<id>/control/pause/set`

| Payload | Action |
| :--- | :--- |
| `true`, `ON`, `1`, `pause` | Pauses charging (`SetStartButton(false)`) |
| `false`, `OFF`, `0`, `resume` | Resumes charging (`SetStartButton(true)`) |

### 4.3 Current Limit Setpoint

**Topic**: `<base>/connector/<id>/control/setpoint_amps/set`

- **Payload**: numeric string between `7` and `32` (e.g. `16`, `16.7`, `24`). Decimal numbers are automatically rounded to the nearest integer.
- **Behavior**:
  - If the station is in `State:A` (Available / Free), greenup-link synchronizes both `CCEl:XX` and `CC:XX` to ensure the internal ATmega registers are synchronized before a car plugs in.
  - In all other states, only `CCEl:XX` is sent, allowing the ATmega to dynamically apply the lowest limit between CCEl, TIC, and circuit breakers.
  - Rejects commands outside 7–32A.

### 4.4 External Grid Voltage Injection

Allows injecting live voltage measured by an external device (e.g. Enedis Linky TIC, Shelly EM, solar inverter) into Green'up Link to obtain exact power and energy calculations instead of using the default 230.0V.

**Global Topic**: `<base>/station/voltage/set`  
**Per-Point Topic**: `<base>/connector/<id>/meter/voltage_v/set`

- **Simple Payload**: floating point string between `100.0` and `300.0` (e.g. `236.4`). Automatically applies a 60-second TTL.
- **JSON Payload with Custom TTL**:
  ```json
  {"voltage": 236.4, "ttl_seconds": 120}
  ```
- **Fallback**: If no external voltage update is received before the TTL expires, the station automatically reverts to `230.0V` default.

---

## 5. Home Assistant Auto-Discovery

When `ha_discovery = true` is enabled in `greenup.toml`, Green'up Link automatically registers the charging station in Home Assistant via the standard MQTT Discovery mechanism.

### Discovered Device & Entities

The device is automatically grouped under **Settings > Devices & Services > MQTT**:

- **Device Name**: `Legrand Green'up (<model>)`
- **Manufacturer**: `Legrand`
- **Firmware Version**: `<link_version> (FW <firmware_version>)`

#### Exposed Entities per Connector:

1. **Sensors**:
   - `Power` (W, device_class: `power`, state_class: `measurement`)
   - `Total Energy` (kWh, device_class: `energy`, state_class: `total_increasing`)
   - `Current` (A, device_class: `current`, state_class: `measurement`)
   - `Voltage` (V, device_class: `voltage`, state_class: `measurement`)
   - `IEC State` (State A, B, C, E, F)
   - `OCPP Status` (Available, Preparing, Charging, Faulted...)
   - `Applied Limit` (A, effective current applied by ATmega)
2. **Binary Sensors**:
   - `Vehicle Connected` (device_class: `plug`)
   - `Charging` (device_class: `battery_charging`)
   - `Session Active` (icon: `mdi:progress-clock`)
3. **Controls (Switches & Numbers)**:
   - `Charge` Switch (`ON` / `OFF`)
   - `Pause` Switch (`true` / `false`)
   - `Current Setpoint` Number Slider (7A – 32A, step 1A)
4. **Station Level Controls**:
   - `Station Total Power` (W)
   - `Grid Voltage Injection` Number (100V – 300V, step 0.5V)
   - `Station Sniffer Mode` Binary Sensor

---

## 6. EVCC MQTT Integration Example

In EVCC (`/etc/evcc.yaml`), you can configure your Legrand Green'up charger using pure MQTT without calling local HTTP scripts:

```yaml
chargers:
  - name: greenup_mqtt
    type: custom

    # 1. IEC Operating State (A, B, C, E, F)
    status:
      source: mqtt
      topic: greenup/connector/1/status/state

    # 2. Real-time Power (Watts)
    power:
      source: mqtt
      topic: greenup/connector/1/meter/active_power_w

    # 3. Total Energy (kWh)
    energy:
      source: mqtt
      topic: greenup/connector/1/meter/energy_total_kwh

    # 4. Charger Enable State
    enabled:
      source: mqtt
      topic: greenup/connector/1/control/charge
      jq: . == "ON"

    # 5. Enable / Disable Command
    enable:
      source: mqtt
      topic: greenup/connector/1/control/charge/set
      payload: ${enable:ON:OFF}

    # 6. Dynamic Current Regulation (Amperes)
    maxcurrent:
      source: mqtt
      topic: greenup/connector/1/control/setpoint_amps/set
```

---

## 7. Home Assistant Automation Example: External Grid Voltage Injection

If you have a Linky TIC sensor or a Shelly EM measuring your home's mains voltage, you can automatically inject it into Green'up Link every minute:

```yaml
alias: "Greenup - Synchronize Grid Voltage"
description: "Injects real-time grid voltage into Legrand Green'up for accurate power calculation"
trigger:
  - platform: time_pattern
    seconds: "/30" # Every 30 seconds
condition:
  - condition: template
    value_template: "{{ states('sensor.linky_voltage') not in ['unavailable', 'unknown'] }}"
action:
  - action: mqtt.publish
    data:
      topic: "greenup/station/voltage/set"
      payload: "{{ states('sensor.linky_voltage') | float(230.0) | round(1) }}"
```
