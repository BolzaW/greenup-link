# DIY Alternative Communication Kit for Legrand Green'up Premium

This document provides a conceptual hardware Bill of Materials (BOM), specifications, and connection schematics to build an **economical DIY alternative communication kit** for the Legrand Green'up Premium charging station, as a direct replacement for the official Legrand kit (reference **059056**, typically sold for ~500 € to 700 €).

This kit is designed to run the lightweight **Green'Up Link** daemon (written in Rust), offering a modern web interface, local REST API, and native integration with Home Assistant and EVCC.

---

> [!CAUTION]
> ### ⚡ Mains Electrical Safety Warning
> Any physical intervention inside the EV charging station must **strictly be performed with the main electrical circuit breaker switched OFF (completely de-energized)**. Fatal electrical shock hazards (**230V single-phase / 400V three-phase**) are present inside the enclosure. Always verify the complete absence of voltage with a verified multimeter before removing the station's front cover.

> [!WARNING]
> ### 🧪 Experimental Project & Proof of Concept Notice
> This DIY communication kit is **experimental**. In practice, **it has never been fully assembled and tested as a complete end-to-end package in real-world conditions**. No 3D-printable mounting bracket model currently exists.
> 
> This document represents a theoretical engineering roadmap and reference guide that is intended to evolve over time through community testing, feedback, and contributions.

---

## 1. Hardware Bill of Materials (BOM)

### A. Single Board Computer (SBC)

Two options are available depending on whether your charging station is within Wi-Fi range or wired with an Ethernet cable:

| Component | Reference / Model | Role | Characteristics & Notes | Indicative Retail Price (2026) |
| :--- | :--- | :--- | :--- | :--- |
| **Option 1 (Recommended - Wi-Fi)** | **Raspberry Pi Zero 2 W** | Runs Linux and the Green'Up Link service. | Ultra-compact, Quad-core 64-bit ARM Cortex-A53, 512 MB RAM, 2.4 GHz Wi-Fi. Generates very little heat (< 2W), making it ideal for the enclosed station. | ~**20 € – 25 €** *(carte seule, e.g. Kubii)* |
| **Option 2 (Full-size Form Factor)** | **Raspberry Pi 3 Model B+** *(or Pi 4)* | Alternative SBC option. | **This is the original physical form factor chosen by Legrand** for their factory communication kit (which uses a Raspberry Pi 3B). Having an integrated RJ45 Ethernet port is simply an added bonus if a network cable is present. | ~**45 € – 55 €** *(carte seule, e.g. Kubii)* |

> [!TIP]
> **Memory & Thermal footprint:** With **Green'Up Link written in Rust**, memory consumption is below 35 MB of RAM and CPU usage is under 1%. The 512 MB RAM of the **Pi Zero 2 W** is more than sufficient and produces minimal heat inside the sealed wallbox during summer months.

---

### B. Power Supply Options (Internal vs. External)

Two methods can be used to power the Raspberry Pi:

#### Option 1: Internal DC-DC Converter (Factory-Style & Fully Integrated)
The official Legrand kit taps directly into the DC voltage provided by the station's internal Mean Well power supply (which delivers **12V DC**, or 24V DC depending on the hardware revision).

To replicate Legrand's setup with off-the-shelf commercial components:

| Component | Specifications | Search Keywords (Amazon / AliExpress / RS) | Indicative Price |
| :--- | :--- | :--- | :--- |
| **Waterproof Buck Converter 12V/24V to 5V 3A (15W)** | - Input: 8V – 32V DC<br>- Output: 5.0V / 3A (15W)<br>- Format: Sealed black resin case (IP67 waterproof)<br>- Output connector: **Micro-USB male** (for Pi Zero 2 W or Pi 3B+) or **USB-C male** (for Pi 4) | *12V to 5V 3A Micro USB buck converter* / *DC-DC Step Down Waterproof 12V 5V 3A* | ~**8 € – 12 €** |

*Advantages:* Completely self-contained inside the wallbox enclosure; no external cable or power outlet required.  
*Requirements:* Requires a 2-pin Molex connector (see Section C below) to tap into the station's internal auxiliary harness.

#### Option 2: External Standard USB Power Supply (Simplified Alternative)
As an easier alternative, you can use a standard official or commercial Raspberry Pi 5V power supply (5.1V / 2.5A or 3A with Micro-USB for Pi Zero 2 W / Pi 3B+, or USB-C for Pi 4). 

The low-voltage power cord is simply routed into the charging station enclosure through one of the waterproof cable entries / glands (*presse-étoupe*) located under the chassis to power the Raspberry Pi.

| Component | Specifications | Notes | Indicative Price |
| :--- | :--- | :--- | :--- |
| **Standard Raspberry Pi Power Supply (5V 2.5A/3A)** | Official Raspberry Pi PSU (or high-quality equivalent) with Micro-USB or USB-C male output | Replaces both the internal DC-DC converter and the Molex connector. | ~**8 € – 12 €** |

*Advantages:* Extremely simple setup with zero soldering or crimping required; no need to tap into the internal station wiring.  
*Requirements:* Requires an external 230V AC mains wall outlet nearby or an electrical extension cord.

---

### C. Connectors & Cabling

| Component | Specification | Role | Indicative Price |
| :--- | :--- | :--- | :--- |
| **Wallbox Auxiliary Power Connector** *(Option 1 only)* | Connector type **Molex Mini-Fit Jr. 2-pin (1x2 single row)** with top locking latch (pitch 4.2 mm, e.g. Molex series 5557 / 39-01-4020 or 39-01-4021) | Plugs directly into the station's internal 12V auxiliary power cable. **Pinout:** Red = **+12V DC**, Black = **GND (0V)**. Pre-crimped pigtails or DIY kits with female pins. *(Not needed if using Option 2 External PSU)*. | ~**2 € – 4 €** |
| **USB Cable for Controller Link** | - For Pi Zero 2 W: **Micro-USB male to USB-A female OTG adapter** + short USB cable.<br>- For Pi 3B+ / 4: Short standard **USB-A to USB cable** connecting to the power board. | Connects the Raspberry Pi to the ATmega2560 power controller board. | ~**3 € – 5 €** |
| **High Endurance MicroSD Card** | **Kingston High Endurance** (32 GB, `SDCE/32GB`) or **Kingston Industrial** (`SDCIT2/32GB`) / **SanDisk MAX Endurance** | System storage. High/Industrial endurance cards are built to withstand continuous writes and extreme operating temperatures (-25°C to +85°C) in outdoor wallboxes. | ~**45 € – 50 €** *(Standard consumer card: ~10 €)* |

> [!NOTE]
> ### 🔌 12V Auxiliary Power Connector Identification & Pinout
> - **Arrangement:** 2-pin single row (**1x2**) with retention clip / positive lock (pitch 4.20 mm).
> - **Polarity / Wiring:**
>   - **Red wire:** **+12V DC** (positive from the station's internal Mean Well power supply)
>   - **Black wire:** **GND / 0V** (ground return)
> - **Important Distinction:** Inside the charging station, an adjacent harness with identical connectors powers other auxiliary modules (such as the optional RFID reader 0 590 59 or TIC/Bluetooth board). Verify that you tap the 2-pin harness with Red/Black wires delivering 12V DC.
> - **Search references:** *Molex Mini-Fit Jr 2 pin 1x2* / *Molex 5557 2P 4.2mm* / *Molex 39-01-4020*. Pre-crimped 2-pin pigtail cables (*"câble Molex 2 broches 4.2mm pré-câblé"*) make connection to the DC-DC buck converter quick and easy without dedicated crimping pliers.

---

### D. Mechanical Mounting & Fasteners

| Component | Status & Specifications | Notes | Indicative Cost |
| :--- | :--- | :--- | :--- |
| **3D Mounting Bracket** | ⚠️ **None currently exists!** | No 3D model (STL/STEP) has been designed or validated yet. A custom bracket remains to be modeled by the community. When designed, it must be printed in **PETG** or **ABS/ASA** (never use PLA, which softens and deforms above 50°C in outdoor enclosures). | ~**2 € – 3 €** *(filament)* |
| **Chassis Fasteners** | **4x M4 screws** (e.g. M4x7 or M4x8) | Per official Legrand installation notice **LE09802AA**, the wallbox internal mounting bosses use **M4 screws** to secure the communication plate. | ~**1 €** |
| **SBC Fasteners** | **4x M2.5 screws and standoffs** | Standard Raspberry Pi mounting hole hardware to secure the board to the bracket. | ~**1 €** |

---

## 2. Indicative Budget Summary

| Item | Official Legrand Kit (Ref 059056) | DIY Alternative Kit (Pi Zero 2 W) | DIY Alternative Kit (Pi 3B+) |
| :--- | :--- | :--- | :--- |
| **SBC / Processor** | Proprietary Raspberry Pi 3B | Raspberry Pi Zero 2 W (~22 €) | Raspberry Pi 3 Model B+ (~46 €) |
| **5V Power Supply** | Proprietary Legrand PCB | Waterproof DC-DC Converter (~10 €)<br>*or Standard External PSU (~10 €)* | Waterproof DC-DC Converter (~10 €)<br>*or Standard External PSU (~10 €)* |
| **Cabling & Connectors** | Included in kit | Molex connector + USB cables (~7 €)<br>*(or USB cable only ~3 € if external PSU)* | Molex connector + USB cable (~5 €)<br>*(or USB cable only ~3 € if external PSU)* |
| **MicroSD Card** | 16 GB standard card | 32 GB High Endurance Kingston (~45 €)<br>*(or standard card ~10 €)* | 32 GB High Endurance Kingston (~45 €)<br>*(or standard card ~10 €)* |
| **Mounting Plate & Screws** | Stamped metal bracket + screws | Custom 3D bracket + M4 / M2.5 screws (~4 €) | Custom 3D bracket + M4 / M2.5 screws (~4 €) |
| **TOTAL** | **~500 € to 700 €** | **~84 € – 88 €** *(~49 € – 53 € with consumer SD)* | **~106 € – 110 €** *(~71 € – 75 € with consumer SD)* |

---

## 3. Synoptic Wiring Diagrams

### Setup A: Internal Power Tap (Fully Integrated)

```text
[ Internal Station Power Supply (12V DC / 24V DC) ]
                        |
                        | (Factory Legrand harness)
                        v
          [ Molex 2-pin (1x2) Connector ]
              |                     |
              | (+12V DC - Red)     | (GND - Black)
              +----------+   +------+
                         |   |
                         v   v
        [ Waterproof DC-DC Buck Converter (12V->5V 3A) ]
                         |
                         | (Regulated 5.0V output via Micro-USB / USB-C)
                         v
              [ Raspberry Pi SBC ] <===== (USB Cable) =====> [ ATmega2560 Power Board ]
               (Green'Up Link)
                      |
                      +-- Local Wi-Fi / Ethernet --> [ Home Automation / EVCC ]
```

### Setup B: External Power Supply (Simplified Alternative)

```text
[ 230V AC Mains Wall Outlet / Extension Cord ]
                        |
                        v
  [ Standard Raspberry Pi USB Power Supply (5V 3A) ]
                        |
                        | (Micro-USB / USB-C routed via bottom cable entry / gland)
                        v
              [ Raspberry Pi SBC ] <===== (USB Cable) =====> [ ATmega2560 Power Board ]
               (Green'Up Link)
                      |
                      +-- Local Wi-Fi / Ethernet --> [ Home Automation / EVCC ]
```
