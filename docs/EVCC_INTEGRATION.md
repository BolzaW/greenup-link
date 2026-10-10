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
Below is the exact `chargers` block required to communicate with Green'Up Link. 

> **Important:** Replace `<GREENUP_IP>` with `127.0.0.1` if EVCC is running on the same Raspberry Pi, or with the actual IP address if EVCC is running remotely.

```yaml
chargers:
  - name: greenup
    type: custom
    
    # 1. Real-time Status (A, B, C, E, F)
    # The API natively returns the exact letter expected by EVCC
    status:
      source: http
      uri: http://<GREENUP_IP>:3000/api/telemetry
      jq: .iec_state
      timeout: 5s
      
    # 2. Real-time Power (Watts)
    power:
      source: http
      uri: http://<GREENUP_IP>:3000/api/telemetry
      jq: .power
      timeout: 5s
      
    # 3. Session Energy (kWh)
    energy:
      source: http
      uri: http://<GREENUP_IP>:3000/api/telemetry
      jq: .energy
      timeout: 5s
      
    # 4. Charger Enable State
    # Combines charge_authorized and !charge_paused directly in the API
    enabled:
      source: http
      uri: http://<GREENUP_IP>:3000/api/telemetry
      jq: .charge_authorized
      timeout: 5s
      
    # 5. Start / Stop Action
    # Script setter with JSON body. EVCC replaces ${enable} with true/false
    enable:
      source: script
      cmd: /bin/sh -c "if [ '${enable}' = 'true' ]; then curl -s -X POST -H 'Content-Type: application/json' -d '{\"action\":\"enable\"}' http://<GREENUP_IP>:3000/api/charge; else curl -s -X POST -H 'Content-Type: application/json' -d '{\"action\":\"disable\"}' http://<GREENUP_IP>:3000/api/charge; fi"
      
    # 6. Current Limiting Action (Amps)
    # Allows EVCC to modulate charging power (e.g., for solar surplus matching)
    maxcurrent:
      source: script
      cmd: curl -s -X POST http://<GREENUP_IP>:3000/api/current/${maxcurrent}
```

### Full Minimal Example

To make EVCC work, you must link the charger to a `loadpoint` and a `site`. Here is a complete minimal `evcc.yaml`:

```yaml
network:
  schema: http
  port: 7070

meters:
  - name: my_grid
    type: template
    template: dummy # REPLACE with your actual grid meter (Shelly, Enphase, Teleinfo...)
    usage: grid

chargers:
  - name: greenup
    # ... (paste the custom block from above here) ...

loadpoints:
  - title: Garage
    charger: greenup
    mode: pv        # 'pv' (solar surplus), 'now' (fast charge), or 'minpv'
    mincurrent: 7   # Minimum 8A for most EVs
    maxcurrent: 32  # Maximum 32A

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

