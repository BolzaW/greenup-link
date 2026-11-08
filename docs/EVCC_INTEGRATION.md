# EVCC Integration Guide

[EVCC](https://evcc.io/) is an open-source smart EV charging controller. It supports solar surplus charging, dynamic electricity pricing, and load balancing. 

Thanks to the robust REST API provided by Green'Up Link, integrating your Legrand charging station with EVCC is fully supported through the EVCC `custom` charger plugin.

---

## 1. Installation Scenarios

You can run EVCC on the same Raspberry Pi that runs Green'Up Link, or on a completely separate remote server (like a Home Assistant machine, a NAS, or a dedicated Linux server).

### Option A: Remote Linux Server (Recommended)
Running EVCC on a dedicated smart-home server is recommended if you already have a 24/7 server managing your solar inverters, grid meters, or home automation.
*   **Networking:** You will need to replace `<GREENUP_IP>` in the configuration with the actual local IP address of your Raspberry Pi (e.g., `192.168.1.50`).

### Option B: Local Raspberry Pi
You can install EVCC directly on the Raspberry Pi located inside the charging station.
*   **Networking:** You can keep `127.0.0.1` as the IP address in the configuration.
*   **Hardware:** EVCC is very lightweight (written in Go) and runs perfectly on a Raspberry Pi Zero 2 W or Raspberry Pi 3/4 alongside Green'Up Link.

---

## 2. Installing EVCC (Linux / Raspberry Pi OS)

For Debian, Ubuntu, or Raspberry Pi OS, the easiest way to install EVCC is via their official APT repository. Run the following commands in your terminal:

```bash
# 1. Add the EVCC repository key
curl -1sLf 'https://dl.evcc.io/public/evcc/cfg/setup/bash.deb.sh' | sudo -E bash

# 2. Install EVCC
sudo apt install evcc
```

*(For other systems like Docker or macOS, refer to the [official EVCC installation guide](https://docs.evcc.io/docs/installation/manual)).*

---

## 3. EVCC Configuration (`evcc.yaml`)

EVCC is configured using a YAML file, usually located at `/etc/evcc.yaml`. 

### 3.1 Single Charge Point Configuration

Below is the standard `chargers` block required to communicate with Green'Up Link for a single-point station (or targeting the default Point 1).

> **Important:** Replace `<GREENUP_IP>` with `127.0.0.1` if EVCC is running on the same Raspberry Pi, or with the actual IP address if EVCC is running remotely.

```yaml
chargers:
  - name: greenup
    type: custom
    
    # 1. Real-time Status (A, B, C, E, F)
    # The API natively returns the exact letter expected by EVCC
    status:
      source: http
      uri: http://<GREENUP_IP>:8080/api/status
      jq: .iec
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
      
    # 2. Real-time Power (Watts)
    power:
      source: http
      uri: http://<GREENUP_IP>:8080/api/meter
      jq: .power_w
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
      
    # 3. Session Energy (kWh)
    energy:
      source: http
      uri: http://<GREENUP_IP>:8080/api/meter
      jq: .energy_kwh
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
      
    # 4. Charger Enable State
    # Combines charge_authorized and !charge_paused directly in the API
    enabled:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/1
      jq: .control.charge_authorized
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
      
    # 5. Start / Stop Action
    # Pure HTTP POST with JSON body. EVCC replaces {{if .enable}} with enable or disable
    enable:
      source: http
      uri: http://<GREENUP_IP>:8080/api/charge
      method: POST
      headers:
        - Content-Type: application/json
        # - Authorization: Bearer <YOUR_API_TOKEN>
      body: '{"action": "{{if .enable}}enable{{else}}disable{{end}}"}'
      timeout: 10s
      
    # 6. Current Limiting Action (Amps)
    # EVCC replaces {{.maxcurrent}} dynamically with the integer target (6 to 32A)
    maxcurrent:
      source: http
      uri: http://<GREENUP_IP>:8080/api/current/{{.maxcurrent}}
      method: POST
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
```

### 3.2 Dual Charge Point Configuration (2 Independent Loadpoints)

If your charging station is a dual-point model (e.g. Legrand references 058011, 058013, 058042, 058044, 059011...), you can define two independent chargers and loadpoints using the dedicated connector endpoints `/api/connectors/1/*` and `/api/connectors/2/*` (or `?side=1` and `?side=2`):

```yaml
chargers:
  # Charge Point 1 (Right / Côté Droit)
  - name: greenup_point1
    type: custom
    status:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/1/status
      jq: .iec
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
    power:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/1/meter
      jq: .power_w
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
    energy:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/1/meter
      jq: .energy_kwh
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
    enabled:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/1
      jq: .control.charge_authorized
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
    enable:
      source: http
      uri: http://<GREENUP_IP>:8080/api/charge?side=1
      method: POST
      headers:
        - Content-Type: application/json
        # - Authorization: Bearer <YOUR_API_TOKEN>
      body: '{"action": "{{if .enable}}enable{{else}}disable{{end}}"}'
      timeout: 10s
    maxcurrent:
      source: http
      uri: http://<GREENUP_IP>:8080/api/current/{{.maxcurrent}}?side=1
      method: POST
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>

  # Charge Point 2 (Left / Côté Gauche)
  - name: greenup_point2
    type: custom
    status:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/2/status
      jq: .iec
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
    power:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/2/meter
      jq: .power_w
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
    energy:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/2/meter
      jq: .energy_kwh
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
    enabled:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/2
      jq: .control.charge_authorized
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>
    enable:
      source: http
      uri: http://<GREENUP_IP>:8080/api/charge?side=2
      method: POST
      headers:
        - Content-Type: application/json
        # - Authorization: Bearer <YOUR_API_TOKEN>
      body: '{"action": "{{if .enable}}enable{{else}}disable{{end}}"}'
      timeout: 10s
    maxcurrent:
      source: http
      uri: http://<GREENUP_IP>:8080/api/current/{{.maxcurrent}}?side=2
      method: POST
      timeout: 10s
      # headers:
      #   - Authorization: Bearer <YOUR_API_TOKEN>

loadpoints:
  - title: Garage Point 1 (Right)
    charger: greenup_point1
    mode: pv
    mincurrent: 6
    maxcurrent: 32

  - title: Garage Point 2 (Left)
    charger: greenup_point2
    mode: pv
    mincurrent: 6
    maxcurrent: 32
```

### 3.3 Off-Peak / Peak Tariffs Configuration (Optional)

If your electricity contract includes Off-Peak / Peak hours (Heures Creuses / Heures Pleines), you can define a time-of-use grid tariff under `tariffs`.

> **Note on Midnight Crossing:** EVCC evaluates daily schedules sequentially. Time spans that cross midnight must be split around `0` (e.g. `22-0` for evening and `0-6` for morning). Do not use `24`.

```yaml
tariffs:
  currency: EUR
  grid:
    type: fixed
    price: 0.2516 # Peak Hours price per kWh (Heures Pleines)
    zones:
      # Evening off-peak segment (e.g., 22:00 to midnight)
      - hours: 22-0
        price: 0.1758 # Off-Peak price per kWh (Heures Creuses)
      # Morning off-peak segment (e.g., midnight to 06:00)
      - hours: 0-6
        price: 0.1758
```

**How to charge automatically during Off-Peak hours:**
1. Set the charger mode to **`pv`** (Solar / Smart mode) in the EVCC Web UI.
2. In the charger settings, set the **Price Limit** (Smart Grid threshold) to your Off-Peak rate (e.g. `0.18 €`).
3. During the day, the charger will only charge if solar surplus is available. At 22:00, the grid rate drops below your threshold, triggering automatic night charging until 06:00.

### 3.4 MQTT-based Configuration

When MQTT is enabled in Green'up Link (`[mqtt]` in `greenup.toml`), you can pilot your charger using native MQTT without running local HTTP requests:

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

For full MQTT topic reference and Home Assistant discovery, see [GREENUP_LINK_MQTT.md](GREENUP_LINK_MQTT.md).

### 3.5 Full Minimal Example (Single Charger)

To make EVCC work, you must link the charger to a `loadpoint` and a `site`. Here is a complete minimal `/etc/evcc.yaml`:

```yaml
network:
  schema: http
  port: 7070

# Replace with your actual grid meter (Shelly, Enphase, Teleinfo / Linky...)
meters:
  - name: my_grid
    type: template
    template: dummy
    usage: grid

chargers:
  - name: greenup
    type: custom
    status:
      source: http
      uri: http://<GREENUP_IP>:8080/api/status
      jq: .iec
      timeout: 10s
    power:
      source: http
      uri: http://<GREENUP_IP>:8080/api/meter
      jq: .power_w
      timeout: 10s
    energy:
      source: http
      uri: http://<GREENUP_IP>:8080/api/meter
      jq: .energy_kwh
      timeout: 10s
    enabled:
      source: http
      uri: http://<GREENUP_IP>:8080/api/connectors/1
      jq: .control.charge_authorized
      timeout: 10s
    enable:
      source: http
      uri: http://<GREENUP_IP>:8080/api/charge
      method: POST
      headers:
        - Content-Type: application/json
      body: '{"action": "{{if .enable}}enable{{else}}disable{{end}}"}'
      timeout: 10s
    maxcurrent:
      source: http
      uri: http://<GREENUP_IP>:8080/api/current/{{.maxcurrent}}
      method: POST
      timeout: 10s

loadpoints:
  - title: Garage
    charger: greenup
    mode: pv        # 'pv' (solar / smart price), 'now' (fast charge), or 'minpv'
    mincurrent: 6   # Minimum 6A (IEC standard)
    maxcurrent: 32  # Maximum 32A (7.4 kW monophase / 22 kW triphase)

site:
  title: My Home
  meters:
    grid: my_grid
```

---

## 4. Starting the Service

Once your `/etc/evcc.yaml` is ready, test the configuration to ensure there are no syntax errors:

```bash
evcc check
```

If everything is green, enable and start the EVCC service:

```bash
sudo systemctl enable evcc
sudo systemctl start evcc
```

You can now open the EVCC Web Interface by navigating to `http://<EVCC_IP>:7070` in your browser.
