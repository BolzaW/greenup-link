# Green'Up Premium Range Compatibility Matrix

This document provides an exhaustive compatibility reference for the Legrand Green'up Premium wallbox range (all sharing the internal ATmega2560 power controller board family).

---

## 🚦 Compatibility Levels Defined

| Level | Description |
| :--- | :--- |
| 🟢 **Full remote control** | Fully supported and controllable via Green'Up Link (Type 2 socket(s), single-phase, no domestic socket). |
| 🟡 **Partial remote control** | Partial support. The Type 2 socket(s) are fully controllable (including both independent sides on dual-point stations), but domestic Schuko sockets are unmanaged (physical contactor only, no software power metering or current modulation). |
| 🔴 **None** | Not supported (Three-phase models). While in theory the ATmega might respond to common serial commands, three-phase power regulation, phase allocation, and safety parameters are completely unverified. |

> [!WARNING]
> **Theoretical compatibility vs real-world validation:**
> In theory, all Legrand Green'up Premium stations share the same internal ATmega2560 microcontroller family and common ASCII communication protocol over USB. Therefore, even three-phase stations *might* technically respond to basic commands.
> 
> However, **Green'Up Link was physically tested and validated on model 0 580 01 (single-point, single-phase, 7.4 kW)**. While single-phase dual-point support is implemented according to Legrand's decompiled Java firmware specifications and USB protocol reverse-engineering, three-phase power regulation, phase balancing, or domestic Schuko contactors are currently unverified.
> 
> If you own one of these models and would like to help test or capture communication logs, please see the [Call for Testers](../README.md#call-for-testers--contributions) section!

---

## 📊 Hardware Reference Matrix

### 1. Plastic Wallbox Range — Single Charge Point (Without Kit)

| Model Name | Legrand Reference | Min Power | Max Power | Electrical Grid | T2 Socket | Schuko Socket | Green'Up Link Compatibility | Details / Limitations |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :--- | :--- |
| Green’up Premium 3.7–4.6 kW T2 | **0 580 00** | 3.7 kW | 4.6 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | Single T2 socket |
| Green’up Premium 3.7–7.4 kW T2 | **0 580 01** | 3.7 kW | 7.4 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | **Tested & fully validated** (Author's hardware) |
| Green’up Premium 11–22 kW T2 | **0 580 02** | 11 kW | 22 kW | Three-phase | 1 | 0 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium 3.7–4.6 kW T2 + schuko | **0 580 03** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko | **0 580 04** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–4.6 kW T2 + schuko (Intl) | **0 580 30** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko (Intl) | **0 580 35** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |

### 2. Plastic Wallbox Range — Single Charge Point (Factory Bundled with IP Kit)

| Model Name | Legrand Reference | Min Power | Max Power | Electrical Grid | T2 Socket | Schuko Socket | Green'Up Link Compatibility | Details / Limitations |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :--- | :--- |
| Green’up Premium 3.7–4.6 kW T2 with kit | **0 590 00** | 3.7 kW | 4.6 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | Factory bundled with KitCom 0 590 56 |
| Green’up Premium 3.7–7.4 kW T2 with kit | **0 590 01** | 3.7 kW | 7.4 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | Factory bundled with KitCom 0 590 56 |
| Green’up Premium 11–22 kW T2 with kit | **0 590 02** | 11 kW | 22 kW | Three-phase | 1 | 0 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium 3.7–4.6 kW T2 + schuko with kit | **0 590 03** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko with kit | **0 590 04** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–4.6 kW T2 with kit | **0 590 05** | 3.7 kW | 4.6 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | Single T2 socket |
| Green’up Premium 3.7–7.4 kW T2 with kit | **0 590 06** | 3.7 kW | 7.4 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | Same as 0 580 01 + 0 590 56 |
| Green’up Premium 11–22 kW T2 with kit | **0 590 07** | 11 kW | 22 kW | Three-phase | 1 | 0 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium 3.7–4.6 kW T2 + schuko with kit | **0 590 08** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko with kit | **0 590 09** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–4.6 kW T2 + schuko with kit (Intl) | **0 590 30** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko with kit (Intl) | **0 590 35** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–4.6 kW T2 + schuko (Without Breaker) | **0 590 70** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko (Without Breaker) | **0 590 71** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |

### 3. Metal Pro Wallbox Range — Single Charge Point (1 Point)

| Model Name | Legrand Reference | Min Power | Max Power | Electrical Grid | T2 Socket | Schuko Socket | Green'Up Link Compatibility | Details / Limitations |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :--- | :--- |
| Green’up Premium metal 3.7–4.6 kW 1 pt T2 + schuko | **0 580 10** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 3.7–7.4 kW 1 pt T2 + schuko | **0 580 12** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 11–22 kW 1 pt T2 + schuko | **0 580 14** | 11 kW | 22 kW | Three-phase | 1 | 1 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium metal 3.7–4.6 kW 1 pt T2 + schuko EV | **0 580 41** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 3.7–7.4 kW 1 pt T2 + schuko EV | **0 580 43** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 11–22 kW 1 pt T2 + schuko EV | **0 580 48** | 11 kW | 22 kW | Three-phase | 1 | 1 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium metal 3.7–4.6 kW 1 pt T2 + schuko with kit | **0 590 10** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 3.7–7.4 kW 1 pt T2 + schuko with kit | **0 590 12** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 11–22 kW 1 pt T2 + schuko with kit | **0 590 14** | 11 kW | 22 kW | Three-phase | 1 | 1 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium metal 3.7–4.6 kW 1 pt T2 + schuko EV with kit | **0 590 41** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 3.7–7.4 kW 1 pt T2 + schuko EV with kit | **0 590 43** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 11–22 kW 1 pt T2 + schuko EV with kit | **0 590 48** | 11 kW | 22 kW | Three-phase | 1 | 1 | 🔴 **None** | Three-phase (untested, uncertain behavior) |

### 4. Metal Pro Wallbox Range — Dual Charge Points (2 Points, Dual ATmega)

> [!NOTE]
> In official Legrand firmware architecture, dual-point stations are equipped with **two independent ATmega2560 microcontroller boards** connected to the Raspberry Pi over two separate internal USB links.
> 
> Green'Up Link provides full multi-loadpoint support with:
> - Automatic hardware reference detection (`dual_point = "auto"`).
> - Dynamic serial port candidate discovery.
> - Automatic side identification (`Side:49` = Right / Point 1, `Side:48`/`Side:50` = Left / Point 2).
> - Independent telemetry, state machine, current control, and session logging for each point.

| Model Name | Legrand Reference | Min Power | Max Power | Electrical Grid | T2 Sockets | Schuko Sockets | Green'Up Link Compatibility | Details / Limitations |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :--- | :--- |
| Green’up Premium metal 3.7–4.6 kW 2 pts T2 + schuko | **0 580 11** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Both T2 points fully supported via dual-serial workers; Schukos not managed |
| Green’up Premium metal 3.7–4.6 kW 2 pts T2 + schuko EV | **0 580 42** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Both T2 points fully supported via dual-serial workers; Schukos not managed |
| Green’up Premium metal 3.7–4.6 kW 2 pts T2 + schuko with kit | **0 590 11** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Both T2 points fully supported via dual-serial workers; Schukos not managed |
| Green’up Premium metal 3.7–4.6 kW 2 pts T2 + schuko EV with kit | **0 590 42** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Both T2 points fully supported via dual-serial workers; Schukos not managed |
| Green’up Premium metal 3.7–7.4 kW 2 pts T2 + schuko | **0 580 13** | 3.7 kW | 7.4 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Both T2 points fully supported via dual-serial workers; Schukos not managed |
| Green’up Premium metal 3.7–7.4 kW 2 pts T2 + schuko EV | **0 580 44** | 3.7 kW | 7.4 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Both T2 points fully supported via dual-serial workers; Schukos not managed |
| Green’up Premium metal 3.7–7.4 kW 2 pts T2 + schuko with kit | **0 590 13** | 3.7 kW | 7.4 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Both T2 points fully supported via dual-serial workers; Schukos not managed |
| Green’up Premium metal 3.7–7.4 kW 2 pts T2 + schuko EV with kit | **0 590 44** | 3.7 kW | 7.4 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Both T2 points fully supported via dual-serial workers; Schukos not managed |
| Green’up Premium metal 11–22 kW 2 pts T2 + schuko | **0 580 15** | 11 kW | 22 kW | Three-phase | 2 | 2 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium metal 11–22 kW 2 pts T2 + schuko EV | **0 580 49** | 11 kW | 22 kW | Three-phase | 2 | 2 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium metal 11–22 kW 2 pts T2 + schuko with kit | **0 590 15** | 11 kW | 22 kW | Three-phase | 2 | 2 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium metal 11–22 kW 2 pts T2 + schuko EV with kit | **0 590 49** | 11 kW | 22 kW | Three-phase | 2 | 2 | 🔴 **None** | Three-phase (untested, uncertain behavior) |

---

## ⚙️ Dual Charge Point Architecture & Configuration

On dual-point charging stations, Green'Up Link provides seamless operation without needing to duplicate endpoints:

### Configuration (`greenup.toml`)
```toml
[serial]
# Dual charge point mode:
# - "auto"  : Automatically detects 1 or 2 charge points based on the station hardware reference (Default)
# - "true"  : Force 2 charge points mode
# - "false" : Force 1 charge point mode
dual_point = "auto"

# Serial ports are optional. If omitted or empty, Green'Up Link automatically discovers available USB ports.
# port = "/dev/ttyUSB0"
# port2 = "/dev/ttyUSB1"
```

### EVCC Multi-Loadpoint Integration
In EVCC, configure two loadpoints pointing to the same Green'Up Link instance, targeting each side using `?side=1` and `?side=2`:

```yaml
chargers:
  - name: greenup_point1
    type: custom
    status:
      source: http
      uri: http://192.168.1.50:8080/api/telemetry?side=1
      jq: .evcc_state
    enabled:
      source: http
      uri: http://192.168.1.50:8080/api/telemetry?side=1
      jq: .charging
    enable:
      source: http
      uri: http://192.168.1.50:8080/api/charge?side=1
      method: POST
      body: '{"action":"start"}'
    maxcurrent:
      source: http
      uri: http://192.168.1.50:8080/api/current/${maxcurrent}?side=1
      method: POST

  - name: greenup_point2
    type: custom
    status:
      source: http
      uri: http://192.168.1.50:8080/api/telemetry?side=2
      jq: .evcc_state
    enabled:
      source: http
      uri: http://192.168.1.50:8080/api/telemetry?side=2
      jq: .charging
    enable:
      source: http
      uri: http://192.168.1.50:8080/api/charge?side=2
      method: POST
      body: '{"action":"start"}'
    maxcurrent:
      source: http
      uri: http://192.168.1.50:8080/api/current/${maxcurrent}?side=2
      method: POST
```

---

## 🔌 Communication Hardware

| Name | Legrand Reference | Role | Compatibility |
| :--- | :--- | :--- | :--- |
| **Official Legrand IP Communication Kit** | **0 590 56** | Embedded Raspberry Pi 3 connected via internal USB cable(s) | 🟢 **Supported** (Original software replaced by Green'Up Link) |
| **DIY Communication Kit** | — | Standard Raspberry Pi (Zero 2 W / 3 / 4) connected via USB cable(s) | 🟢 **Supported** (See [KIT_COMMUNICATION_DIY.md](KIT_COMMUNICATION_DIY.md)) |
