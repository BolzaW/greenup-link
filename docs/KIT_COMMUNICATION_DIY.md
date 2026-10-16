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

### B. DC-DC Power Supply Module

The official Legrand kit taps directly into the DC voltage provided by the station's internal Mean Well power supply (which delivers **12V DC**, or 24V DC depending on the hardware revision).

To replace Legrand's proprietary power supply daughterboard with an off-the-shelf commercial unit:

| Component | Specifications | Search Keywords (Amazon / AliExpress / RS) | Indicative Price |
| :--- | :--- | :--- | :--- |
| **Waterproof Buck Converter 12V/24V to 5V 3A (15W)** | - Input: 8V – 32V DC<br>- Output: 5.0V / 3A (15W)<br>- Format: Sealed black resin case (IP67 waterproof)<br>- Output connector: **Micro-USB male** (for Pi Zero 2 W or Pi 3B+) or **USB-C male** (for Pi 4) | *12V to 5V 3A Micro USB buck converter* / *DC-DC Step Down Waterproof 12V 5V 3A* | ~**8 € – 12 €** |

*Common proven models: CPT Step-Down Converter, DROK Waterproof DC-DC Buck Converter 12V to 5V Micro-USB.*

---

### C. Connectors & Cabling

To connect the kit to the wallbox power rail **without cutting or modifying Legrand's factory wiring harness**:

| Component | Specification | Role | Indicative Price |
| :--- | :--- | :--- | :--- |
| **Wallbox Auxiliary Power Connector** | Connector type **Molex Mini-Fit Jr.** (4-pin, 2x2 arrangement) | Plugs directly into the station's auxiliary power output connector. | ~**3 € – 4 €** |
| **USB Cable for Controller Link** | - For Pi Zero 2 W: **Micro-USB male to USB-A female OTG adapter** + short USB cable.<br>- For Pi 3B+ / 4: Short standard **USB-A to USB cable** connecting to the power board. | Connects the Raspberry Pi to the ATmega2560 power controller board. | ~**3 € – 5 €** |
| **High Endurance MicroSD Card** | **Kingston High Endurance** (32 GB, `SDCE/32GB`) or **Kingston Industrial** (`SDCIT2/32GB`) / **SanDisk MAX Endurance** | System storage. High/Industrial endurance cards are built to withstand continuous writes and extreme operating temperatures (-25°C to +85°C) in outdoor wallboxes. | ~**45 € – 50 €** *(Standard consumer card: ~10 €)* |

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
| **5V Power Supply** | Proprietary Legrand PCB | Waterproof DC-DC Converter 12V->5V 3A (~10 €) | Waterproof DC-DC Converter 12V->5V 3A (~10 €) |
| **Cabling & Connectors** | Included in kit | Molex Mini-Fit Jr. + USB cables (~7 €) | Molex Mini-Fit Jr. + USB cable (~5 €) |
| **MicroSD Card** | 16 GB standard card | 32 GB High Endurance Kingston (~45 €)<br>*(or standard card ~10 €)* | 32 GB High Endurance Kingston (~45 €)<br>*(or standard card ~10 €)* |
| **Mounting Plate & Screws** | Stamped metal bracket + screws | Custom 3D bracket + M4 / M2.5 screws (~4 €) | Custom 3D bracket + M4 / M2.5 screws (~4 €) |
| **TOTAL** | **~500 € to 700 €** | **~88 €** *(~53 € with consumer SD)* | **~110 €** *(~75 € with consumer SD)* |

---

## 3. Synoptic Wiring Diagram

```text
[ Internal Station Power Supply (12V DC / 24V DC) ]
                        |
                        | (Factory Legrand harness)
                        v
          [ Molex 4-pin (2x2) Connector ]
              |                     |
              | (+12V / +24V DC)    | (GND / Return)
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
