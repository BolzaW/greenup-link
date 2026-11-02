# Green'Up Premium Range Compatibility Matrix

This document provides an exhaustive compatibility reference for the Legrand Green'up Premium wallbox range (all sharing the internal ATmega2560 power controller board family).

---

## 🚦 Compatibility Levels Defined

| Level | Description |
| :--- | :--- |
| 🟢 **Full remote control** | Fully supported, tested, and validated on real hardware (Single-phase, 1x Type 2 socket, no domestic socket). |
| 🟡 **Partial remote control** | Partial support. The Type 2 socket is controllable, but domestic Schuko sockets are unmanaged (physical contactor only, no software power metering or current modulation). |
| 🧪 **Theoretical support (Untested)** | Architecture implemented in software based on reverse engineering of Legrand's official Java sources, but **not physically tested on real hardware** (dual-point charging stations). The author does not own this equipment. Real-world tester feedback is warmly welcome! |
| 🔴 **None** | Not supported (Three-phase models). While in theory the ATmega might respond to common serial commands, three-phase power regulation, phase allocation, and safety parameters are completely unverified. |

> [!WARNING]
> **Real-World Validation vs Theoretical Support:**
> - **Physically tested & fully validated:** Model **0 580 01** (Single-point, single-phase, 7.4 kW without domestic socket — author's hardware).
> - **Theoretical implementation (Untested on real hardware):** Dual-point charging stations (2x Type 2) have been fully coded in software according to the decompiled Legrand Java sources and internal dual-USB architecture. However, **the author does not possess a dual-point wallbox and cannot physically test them**. This feature is therefore considered **theoretical and experimental until verified by a community tester**.
> - **Unverified / Unsupported:** Three-phase stations (11–22 kW) and domestic Schuko contactors.
> 
> If you own a dual-point station or an untested model, we would love your help to test and share serial logs! Please see the [Call for Testers](../README.md#call-for-testers--contributions) section.

---

## ⚡ TIC Linky Hardware Compatibility (French Market)

> [!NOTE]
> **What is TIC?** 
> The **TIC (Télé-Information Client)** is a one-way digital data link specific to French electricity meters (Enedis Linky and legacy electronic meters) communicating via terminals I1/I2 to terminals 6/7 of the charging station board.
> 
> As documented in official Legrand technical literature (*"Livre Blanc — Bornes de recharge : Activation de la gestion de puissance via la TIC Linky"*, Doc EXB22062, July 2023), **only 13 specific charging station references** are physically equipped with and certified for TIC hardware integration. 
> 
> International models, export variants, tertiary dual-point stations, and certain factory bundles do not include TIC input terminals.

---

## 📊 Hardware Reference Matrix

### 1. Plastic Wallbox Range — Single Charge Point (Without Kit)

| Model Name | Legrand Reference | Min Power | Max Power | Electrical Grid | T2 Socket | Schuko Socket | TIC (Linky) | Green'Up Link Compatibility | Details / Limitations |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :---: | :--- | :--- |
| Green’up Premium 3.7–4.6 kW T2 | **0 580 00** | 3.7 kW | 4.6 kW | Single-phase | 1 | 0 | ✅ Yes | 🟢 **Full remote control** | Single T2 socket |
| Green’up Premium 3.7–7.4 kW T2 | **0 580 01** | 3.7 kW | 7.4 kW | Single-phase | 1 | 0 | ✅ Yes | 🟢 **Full remote control** | **Tested & fully validated** (Author's hardware) |
| Green’up Premium 11–22 kW T2 | **0 580 02** | 11 kW | 22 kW | Three-phase | 1 | 0 | ✅ Yes | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium 3.7–4.6 kW T2 + schuko | **0 580 03** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ✅ Yes | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko | **0 580 04** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ✅ Yes | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–4.6 kW T2 + schuko (Intl) | **0 580 30** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium 3.7–7.4 kW T2 + schuko (Intl) | **0 580 35** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |

### 2. Plastic Wallbox Range — Single Charge Point (Factory Bundled with IP Kit)

| Model Name | Legrand Reference | Min Power | Max Power | Electrical Grid | T2 Socket | Schuko Socket | TIC (Linky) | Green'Up Link Compatibility | Details / Limitations |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :---: | :--- | :--- |
| Green’up Premium 3.7–4.6 kW T2 with kit | **0 590 00** | 3.7 kW | 4.6 kW | Single-phase | 1 | 0 | ❌ No | 🟢 **Full remote control** | Factory bundled with KitCom 0 590 56; No TIC |
| Green’up Premium 3.7–7.4 kW T2 with kit | **0 590 01** | 3.7 kW | 7.4 kW | Single-phase | 1 | 0 | ❌ No | 🟢 **Full remote control** | Factory bundled with KitCom 0 590 56; No TIC |
| Green’up Premium 11–22 kW T2 with kit | **0 590 02** | 11 kW | 22 kW | Three-phase | 1 | 0 | ❌ No | 🔴 **None** | Three-phase (untested, uncertain behavior); No TIC |
| Green’up Premium 3.7–4.6 kW T2 + schuko with kit | **0 590 03** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium 3.7–7.4 kW T2 + schuko with kit | **0 590 04** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium 3.7–4.6 kW T2 with kit | **0 590 05** | 3.7 kW | 4.6 kW | Single-phase | 1 | 0 | ✅ Yes | 🟢 **Full remote control** | Single T2 socket |
| Green’up Premium 3.7–7.4 kW T2 with kit | **0 590 06** | 3.7 kW | 7.4 kW | Single-phase | 1 | 0 | ✅ Yes | 🟢 **Full remote control** | Same as 0 580 01 + 0 590 56 |
| Green’up Premium 11–22 kW T2 with kit | **0 590 07** | 11 kW | 22 kW | Three-phase | 1 | 0 | ✅ Yes | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium 3.7–4.6 kW T2 + schuko with kit | **0 590 08** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ✅ Yes | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–7.4 kW T2 + schuko with kit | **0 590 09** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ✅ Yes | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium 3.7–4.6 kW T2 + schuko with kit (Intl) | **0 590 30** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium 3.7–7.4 kW T2 + schuko with kit (Intl) | **0 590 35** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium 3.7–4.6 kW T2 + schuko (Without Breaker) | **0 590 70** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium 3.7–7.4 kW T2 + schuko (Without Breaker) | **0 590 71** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |

### 3. Metal Pro Wallbox Range — Single Charge Point (1 Point)

| Model Name | Legrand Reference | Min Power | Max Power | Electrical Grid | T2 Socket | Schuko Socket | TIC (Linky) | Green'Up Link Compatibility | Details / Limitations |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :---: | :--- | :--- |
| Green’up Premium metal 3.7–4.6 kW 1 pt T2 + schuko | **0 580 10** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ✅ Yes | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 3.7–7.4 kW 1 pt T2 + schuko | **0 580 12** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ✅ Yes | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed |
| Green’up Premium metal 11–22 kW 1 pt T2 + schuko | **0 580 14** | 11 kW | 22 kW | Three-phase | 1 | 1 | ✅ Yes | 🔴 **None** | Three-phase (untested, uncertain behavior) |
| Green’up Premium metal 3.7–4.6 kW 1 pt T2 + schuko EV | **0 580 41** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium metal 3.7–7.4 kW 1 pt T2 + schuko EV | **0 580 43** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium metal 11–22 kW 1 pt T2 + schuko EV | **0 580 48** | 11 kW | 22 kW | Three-phase | 1 | 1 | ❌ No | 🔴 **None** | Three-phase (untested, uncertain behavior); No TIC |
| Green’up Premium metal 3.7–4.6 kW 1 pt T2 + schuko with kit | **0 590 10** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium metal 3.7–7.4 kW 1 pt T2 + schuko with kit | **0 590 12** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium metal 11–22 kW 1 pt T2 + schuko with kit | **0 590 14** | 11 kW | 22 kW | Three-phase | 1 | 1 | ❌ No | 🔴 **None** | Three-phase (untested, uncertain behavior); No TIC |
| Green’up Premium metal 3.7–4.6 kW 1 pt T2 + schuko EV with kit | **0 590 41** | 3.7 kW | 4.6 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium metal 3.7–7.4 kW 1 pt T2 + schuko EV with kit | **0 590 43** | 3.7 kW | 7.4 kW | Single-phase | 1 | 1 | ❌ No | 🟡 **Partial remote control** | Type 2 controllable; Schuko not managed; No TIC |
| Green’up Premium metal 11–22 kW 1 pt T2 + schuko EV with kit | **0 590 48** | 11 kW | 22 kW | Three-phase | 1 | 1 | ❌ No | 🔴 **None** | Three-phase (untested, uncertain behavior); No TIC |

### 4. Metal Pro Wallbox Range — Dual Charge Points (2 Points, Dual ATmega)

> [!IMPORTANT]
> **Theoretical Implementation — Call for Real-Hardware Testers:**
> In official Legrand firmware architecture, dual-point stations are equipped with **two independent ATmega2560 microcontroller boards** connected to the Raspberry Pi over two separate internal USB links.
> 
> Green'Up Link provides full software support (dual serial workers, automatic USB discovery, dynamic side assignment `Side?`, isolated session recording, and EVCC dual loadpoints).
> 
> However, **the author does not own a dual-point station and cannot physically test this feature**. It is therefore presented as **theoretical and experimental**. If you own one of these stations, we would be delighted to receive your feedback and logs!

| Model Name | Legrand Reference | Min Power | Max Power | Electrical Grid | T2 Sockets | Schuko Sockets | TIC (Linky) | Green'Up Link Compatibility | Details / Limitations |
| :--- | :--- | :--- | :--- | :--- | :---: | :---: | :---: | :--- | :--- |
| Green’up Premium metal 3.7–4.6 kW 2 pts T2 + schuko | **0 580 11** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | ❌ No | 🧪 **Theoretical (Untested)** | Dual-serial workers implemented from Legrand specs; unverified on real hardware; Schukos not managed; No TIC |
| Green’up Premium metal 3.7–4.6 kW 2 pts T2 + schuko EV | **0 580 42** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | ❌ No | 🧪 **Theoretical (Untested)** | Dual-serial workers implemented from Legrand specs; unverified on real hardware; Schukos not managed; No TIC |
| Green’up Premium metal 3.7–4.6 kW 2 pts T2 + schuko with kit | **0 590 11** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | ❌ No | 🧪 **Theoretical (Untested)** | Dual-serial workers implemented from Legrand specs; unverified on real hardware; Schukos not managed; No TIC |
| Green’up Premium metal 3.7–4.6 kW 2 pts T2 + schuko EV with kit | **0 590 42** | 3.7 kW | 4.6 kW | Single-phase | 2 | 2 | ❌ No | 🧪 **Theoretical (Untested)** | Dual-serial workers implemented from Legrand specs; unverified on real hardware; Schukos not managed; No TIC |
| Green’up Premium metal 3.7–7.4 kW 2 pts T2 + schuko | **0 580 13** | 3.7 kW | 7.4 kW | Single-phase | 2 | 2 | ❌ No | 🧪 **Theoretical (Untested)** | Dual-serial workers implemented from Legrand specs; unverified on real hardware; Schukos not managed; No TIC |
| Green’up Premium metal 3.7–7.4 kW 2 pts T2 + schuko EV | **0 580 44** | 3.7 kW | 7.4 kW | Single-phase | 2 | 2 | ❌ No | 🧪 **Theoretical (Untested)** | Dual-serial workers implemented from Legrand specs; unverified on real hardware; Schukos not managed; No TIC |
| Green’up Premium metal 3.7–7.4 kW 2 pts T2 + schuko with kit | **0 590 13** | 3.7 kW | 7.4 kW | Single-phase | 2 | 2 | ❌ No | 🧪 **Theoretical (Untested)** | Dual-serial workers implemented from Legrand specs; unverified on real hardware; Schukos not managed; No TIC |
| Green’up Premium metal 3.7–7.4 kW 2 pts T2 + schuko EV with kit | **0 590 44** | 3.7 kW | 7.4 kW | Single-phase | 2 | 2 | ❌ No | 🧪 **Theoretical (Untested)** | Dual-serial workers implemented from Legrand specs; unverified on real hardware; Schukos not managed; No TIC |
| Green’up Premium metal 11–22 kW 2 pts T2 + schuko | **0 580 15** | 11 kW | 22 kW | Three-phase | 2 | 2 | ❌ No | 🔴 **None** | Three-phase (untested, uncertain behavior); No TIC |
| Green’up Premium metal 11–22 kW 2 pts T2 + schuko EV | **0 580 49** | 11 kW | 22 kW | Three-phase | 2 | 2 | ❌ No | 🔴 **None** | Three-phase (untested, uncertain behavior); No TIC |
| Green’up Premium metal 11–22 kW 2 pts T2 + schuko with kit | **0 590 15** | 11 kW | 22 kW | Three-phase | 2 | 2 | ❌ No | 🔴 **None** | Three-phase (untested, uncertain behavior); No TIC |
| Green’up Premium metal 11–22 kW 2 pts T2 + schuko EV with kit | **0 590 49** | 11 kW | 22 kW | Three-phase | 2 | 2 | ❌ No | 🔴 **None** | Three-phase (untested, uncertain behavior); No TIC |

---

## 🔌 Communication Hardware

> [!IMPORTANT]
> **Prerequisite for Green'Up Link operation:**
> A Legrand Green'Up Premium charging station is **compatible with Green'Up Link if and only if** it is equipped with a communication kit. Green'Up Link is not installed onto the station's internal ATmega2560 power board, but runs directly on the communication kit's Raspberry Pi.
> - **Option 1 (Official Kit):** The official Legrand IP Communication Kit (catalog reference **0 590 56**), sold as an optional add-on by Legrand.
> - **Option 2 (DIY Alternative):** A custom DIY kit assembled with a standard Raspberry Pi (Pi Zero 2 W, Pi 3, Pi 4, Pi 5) running standard 64-bit Raspberry Pi OS Lite, connected via USB cable (see the [DIY Communication Kit Guide](KIT_COMMUNICATION_DIY.md)).

### Official Legrand Communication Kits (KitCom) & Factory OS Images

There are many unknowns regarding the various Linux OS versions shipped by Legrand on the official communication kit over the years. Crucially, **the underlying operating system is never maintained or upgraded by Legrand's official software update mechanism**. Legrand update archives only deploy Java JAR/WAR files, scripts, and select daemon packages, but never execute a full operating system upgrade (`dist-upgrade`). Consequently, **the factory OS image on a given kit remains frozen in the exact state it was originally flashed and delivered by Legrand**.

#### Factory Image Release Hypothesis (Author Theory)
A plausible hypothesis is that Legrand may have shipped a distinct factory OS image corresponding to each major official update release. Under this theory, Legrand qualified each update package on an updated base OS, and then flashed that newer OS image onto all physical communication kits manufactured from that qualification date until the next release. However, this remains an unconfirmed hypothesis.

#### Official KitCom Archives Matrix

The table below lists all 7 known official Legrand KitCom release archives identified in the field:

| KitCom Version / Archive | Communication Software | Base OS | Apache Version | glibc Version | Compatibility with Green'Up Link | Notes |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **KitCom V01.15.00-legacy** (`KitCom-V01.15.00.zip`) | `SoftwareCommunication-V01.03.25.jar` | **Raspbian GNU/Linux 9 (Stretch) 32-bit** (Kernel 4.9, `armv7l`) | **2.4.25** | **2.24** | 🟢 **Fully Tested & Supported** | Author's baseline kit; fully operational with `armv7-legacy` binary |
| **KitCom V01.15.00** (`KitCom.V01.15.00.zip`) | `SoftwareCommunication-V01.04.09.jar` | ❓ *Unknown* (likely Raspbian 9) | ❓ *Unknown* (likely 2.4.25) | ❓ *Unknown* (likely 2.24) | ❓ **Untested** | Distinct official archive from legacy; likely compatible |
| **KitCom V01.17.00** (`KitCom.V01.17.00.zip`) | `SoftwareCommunication-V01.16.02.jar` | ❓ *Unknown* | ❓ *Unknown* (likely upgraded) | ❓ *Unknown* | ❓ **Untested** | Intermediate V01.17 release; exact OS unverified |
| **KitCom V01.17.28** (`KitCom.V01.17.28.zip`) | `SoftwareCommunication-V01.17.27.jar` | ❓ *Unknown* (likely Debian 11 transition) | **≥ 2.4.56** *(via deb11 patch)* | ❓ *Unknown* | ❓ **Untested** | Includes `apache2_2.4.56-1~deb11u2_armhf.deb` package update |
| **KitCom V01.18.04** (`KitCom-V01.18.04.zip`) | `SoftwareCommunication-V01.18.04.jar` | ❓ *Unknown* | **≥ 2.4.62** *(via deb11 patch)* | ❓ *Unknown* | ❓ **Untested** | Includes `apache2_2.4.62-1~deb11u2_armhf.deb` package update |
| **KitCom V02.00.02 ("Gen2 Initial")** (`KitCom-V02.00.02.zip`) | `SoftwareCommunication-V02.00.02.jar` *(approx.)* | ❓ *Unknown* (modern Debian / LUKS) | ❓ *Unknown* | ❓ *Unknown* | ❓ **Untested** | Initial baseline image for Gen2; referenced in V02.01.14 release notes & configured in Azure blob storage (`current.json`) |
| **KitCom V02.01.14 ("Gen2")** (`KitCom-V02.01.14.zip`) | `SoftwareCommunication-V02.01.14.jar` | ❓ *Unknown* (modern Debian / LUKS) | **≥ 2.4.62** *(via deb11 patch)* | ❓ *Unknown* | ❓ **Untested** | Major architecture redesign: LUKS encryption (`/run/bringup`), `bticino` user, Tomcat 9, new ZIP password |

> [!NOTE]
> - From version `V01.17.00` onwards, it is assumed that the Apache HTTP server and system libraries were upgraded compared to the baseline `2.4.25`.
> - Notice that there are two distinct official archives for `V01.15.00`: the earlier baseline `KitCom-V01.15.00.zip` (with hyphen `-`, containing `SoftwareCommunication-V01.03.25.jar`), designated here as `1.15.0-legacy`, and `KitCom.V01.15.00.zip` (with dot `.`, containing `SoftwareCommunication-V01.04.09.jar`).
> - The compatibility of factory KitCom images other than `1.15.0-legacy` remains untested and unknown at this stage.

### 🤝 Call for Contributions: Help Us Map All KitCom Images

We are actively seeking feedback from the community to map all physical KitCom versions in circulation!
If you have an official Legrand KitCom running a different version, you can help by connecting via SSH and sharing the output of:
```bash
cat /etc/os-release
uname -a
ldd --version
apache2 -v
```

**Need a dedicated build?**
If a new or different factory OS version turns out to be incompatible with the current `armv7-legacy` binary (due to glibc version mismatches or architectural differences), **the build pipeline will be adapted to create a dedicated release binary** tailored specifically to that system. Please open an issue on GitHub with your system information!
