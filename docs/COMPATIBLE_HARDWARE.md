# Green'Up Premium Range Compatibility Matrix

This document provides an exhaustive compatibility reference for the Legrand Green'up Premium wallbox range (all sharing the internal ATmega2560 power controller board family).

---

## 🚦 Compatibility Levels Defined

| Level | Description |
| :--- | :--- |
| 🟢 **Full remote control** | Fully supported and controllable via Green'Up Link (1x Type 2 socket, single-phase, no domestic socket). |
| 🟡 **Partial remote control** | Partial support. The Type 2 socket is controllable, but the domestic Schuko socket is unmanaged, or only a single side is managed on dual-point stations. |
| 🔴 **None** | Not supported (Three-phase models). While in theory the ATmega might respond to common serial commands, three-phase power regulation, phase allocation, and safety parameters are completely unverified. |

> [!WARNING]
> **Theoretical compatibility vs real-world validation:**
> In theory, all Legrand Green'up Premium stations share the same internal ATmega2560 microcontroller family and common ASCII serial protocol. Therefore, even three-phase stations or models with domestic sockets *might* technically respond to basic commands.
> 
> However, **I have only been able to physically test and validate Green'Up Link on my own hardware (model 0 580 01)**. I am genuinely not sure how three-phase power regulation, phase balancing, or domestic Schuko contactors behave in practice, and there are currently no telemetry mappings or validation for them.
> 
> If you own one of these models and would like to help test or capture serial communication logs, please see the [Call for Testers](../README.md#call-for-testers--contributions) section!

---

## 📊 Hardware Reference Matrix

| Model Name | Legrand Reference | Min Power | Max Power | Electrical Grid | T2 Socket | Schuko Socket | Green'Up Link Compatibility | Details / Limitations |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :--- | :--- |
| Green’up Premium 3.7–4.6 kW T2 | **0 580 00** | 3.7 kW | 4.6 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | Single T2 socket |
| Green’up Premium 3.7–7.4 kW T2 | **0 580 01** | 3.7 kW | 7.4 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | **Tested & fully validated** (Author's hardware) |
| Green’up Premium 11–22 kW T2 | **0 580 02** | 11 kW | 22 kW | Three-phase | 1 | 0 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium 3.7–4.6 kW T2 + schuko | **0 580 03** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko | **0 580 04** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium Pro metal 3.7–4.6 kW 1 charge point T2 + schuko | **0 580 10** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium Pro metal 3.7–4.6 kW 2 charge points T2 + schuko | **0 580 11** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Only 1 side supported; Schuko not managed |
| Green’up Premium Pro metal 3.7–7.4 kW 1 charge point T2 + schuko | **0 580 12** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium Pro metal 3.7–7.4 kW 2 charge points T2 + schuko | **0 580 13** | 3.7 kW | 7.4 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Only 1 side supported; Schuko not managed |
| Green’up Premium Pro metal 11–22 kW 1 charge point T2 + schuko | **0 580 14** | 11 kW | 22 kW | Three-phase | 1 | 1 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium Pro metal 11–22 kW 2 charge points T2 + schuko | **0 580 15** | 11 kW | 22 kW | Three-phase | 2 | 2 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium 3.7–4.6 kW T2 + schuko (Intl) | **0 580 30** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko (Intl) | **0 580 35** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium Pro metal 3.7–4.6 kW 1 charge point T2 + schuko (Intl) | **0 580 41** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium Pro metal 3.7–4.6 kW 2 charge points T2 + schuko (Intl) | **0 580 42** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | 🟡 **Partial remote control** | Only 1 side supported; Schuko not managed |
| Green’up Premium Pro metal 3.7–7.4 kW 1 charge point T2 + schuko (Intl) | **0 580 43** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium Pro metal 11–22 kW 1 charge point T2 + schuko (Intl) | **0 580 48** | 11 kW | 22 kW | Three-phase | 1 | 1 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium Pro metal 11–22 kW 2 charge points T2 + schuko (Intl) | **0 580 49** | 11 kW | 22 kW | Three-phase | 2 | 2 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium 3.7–4.6 kW T2 with kit | **0 590 05** | 3.7 kW | 4.6 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | Single T2 socket |
| Green’up Premium 3.7–7.4 kW T2 with kit | **0 590 06** | 3.7 kW | 7.4 kW | Single-phase | 1 | 0 | 🟢 **Full remote control** | Factory bundled with KitCom (same as 058001 + 059056) |
| Green’up Premium 11–22 kW T2 with kit | **0 590 07** | 11 kW | 22 kW | Three-phase | 1 | 0 | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium 3.7–4.6 kW T2 + schuko with kit | **0 590 08** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko with kit | **0 590 09** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–4.6 kW T2 + schuko (Without Breaker) | **0 590 70** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko (Without Breaker) | **0 590 71** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |

---

## 🔌 Communication Hardware

| Name | Legrand Reference | Role | Compatibility |
| :--- | :--- | :--- | :--- |
| **Official Legrand IP Communication Kit** | **0 590 56** | Embedded Raspberry Pi 3 with RS-232 interface | 🟢 **Supported** (Original hardware replaced by Green'Up Link) |
| **DIY Communication Kit** | — | Raspberry Pi Zero 2 W / 3 / 4 with DIY serial converter | 🟢 **Supported** (See [KIT_COMMUNICATION_DIY.md](KIT_COMMUNICATION_DIY.md)) |
