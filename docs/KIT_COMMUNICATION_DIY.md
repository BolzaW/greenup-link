# DIY Alternative Communication Kit for Legrand Green'up Premium

This document provides a conceptual hardware Bill of Materials (BOM), specifications, and connection schematics to build an **economical DIY alternative communication kit** for the Legrand Green'up Premium charging station, as a direct replacement for the official Legrand kit (reference **059056**, typically sold for ~500 € to 700 €).

This kit is designed to run the lightweight **Green'Up Link** daemon (written in Rust), offering a modern web interface, local REST API, and native integration with Home Assistant and EVCC.

---

> [!CAUTION]
> ### ⚡ Mains Electrical Safety Warning
> Any physical intervention inside the EV charging station must **strictly be performed with the main electrical circuit breaker switched OFF (completely de-energized)**. Fatal electrical shock hazards (**230V single-phase / 400V three-phase**) are present inside the enclosure. Always verify the complete absence of voltage with a verified multimeter before removing the station's front cover.

> [!IMPORTANT]
> ### ✅ Hardware Implementation Status: Field-Validated (Kit V1)
> As of November 2026, this DIY alternative communication kit (Kit V1) has been **fully assembled, physically integrated inside the original station chassis, and successfully field-validated** with a Raspberry Pi 3 Model B+. The system powers on reliably from the internal 12V auxiliary DC supply, fits cleanly in the upper left section of the lateral side panel (the exact location of the official kit), and establishes robust bi-directional serial USB communication with the ATmega2560 controller board.

---

## 1. Hardware Bill of Materials (BOM)

### A. Single Board Computer (SBC)

Two main SBC options can be used depending on your budget, form factor preference, and spare hardware availability:

| Component | Reference / Model | Role | Characteristics & Notes | Indicative Retail Price (2026) |
| :--- | :--- | :--- | :--- | :--- |
| **Option 1 (Recommended)** | **Raspberry Pi Zero 2 W** | Runs Linux and the Green'Up Link service. | **The most cost-effective and compact choice.** Quad-core 64-bit ARM Cortex-A53, 512 MB RAM, integrated Wi-Fi. Its ultra-compact size and minimal heat dissipation (< 2W) make it more than sufficient to run Green'Up Link comfortably. | ~**20 € – 25 €** *(board only, e.g. Kubii)* |
| **Option 2 (Full-size Form Factor)** | **Raspberry Pi 3 Model B+** *(or Pi 3B / Pi 4)* | Alternative SBC option. | **The original factory form factor chosen by Legrand** (the official kit uses a Pi 3B). It fits directly into the side panel cavity, provides an onboard RJ45 Ethernet port if a wired network connection is desired, and is ideal if you already have one available. | ~**45 € – 55 €** *(board only, e.g. Kubii)* |

> [!TIP]
> **Memory & Thermal footprint:** With **Green'Up Link written in Rust**, memory consumption is below 35 MB of RAM and CPU usage is under 1%. The 512 MB RAM of the **Pi Zero 2 W** is more than sufficient and produces minimal heat inside the sealed wallbox during summer months.

---

### B. Power Supply Options (Internal vs. External)

Two methods can be used to power the Raspberry Pi:

#### Option 1: Internal DC-DC Converter (Factory-Style & Fully Integrated)
The official Legrand kit taps directly into the 12V DC voltage provided by the station's internal Mean Well power supply.

To replicate Legrand's setup with off-the-shelf commercial components:

| Component | Specifications | Search Keywords (Amazon / AliExpress / RS) | Indicative Price |
| :--- | :--- | :--- | :--- |
| **Waterproof Buck Converter 12V to 5V 3A (15W)** | - Input: 8V – 32V DC (12V DC nominal)<br>- Output: 5.0V / 3A (15W)<br>- Format: Sealed black resin case (IP67 waterproof)<br>- Output connector: **Micro-USB male** (for Pi Zero 2 W or Pi 3B+) or **USB-C male** (for Pi 4)<br>- *Field-validated model:* **Fastsaw DC-DC 15W Car Power Technology (12V in, 5V/3A out)** | *12V to 5V 3A Micro USB buck converter* / *DC-DC Step Down Waterproof 12V 5V 3A Fastsaw* | ~**8 €** |

*Advantages:* Completely self-contained inside the wallbox enclosure; no external cable or power outlet required.  
*Requirements:* Requires a 2-pin Molex connector and splicing connectors (see Section C below) to tap into the station's internal auxiliary harness.

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
| **Wallbox Auxiliary Power Connector** *(Option 1 only)* | Connector type **Molex Mini-Fit Jr. 2-pin (1x2 single row)** with top locking latch (pitch 4.2 mm, e.g. Molex series 5557 / 39-01-4020)<br>*Field-tested hack:* **Repurposed PC PCIe 6+2 pin cable** (see Pro-Tip below) | Plugs directly into the station's internal 12V auxiliary power cable. **Pinout:** Red = **+12V DC**, Black = **GND (0V)**. *(Not needed if using Option 2 External PSU)*. | ~**2 € – 5 €** *(or free from spare PC parts)* |
| **Splicing Connectors** *(Option 1 only)* | **2x WAGO 221-412** (compact 2-conductor lever connectors) | Safe, reusable, and vibration-resistant connection between the Molex harness and the DC-DC buck converter input wires. | ~**1.50 €** *(pack of 10 ~7 € at hardware stores)* |
| **USB Cable for Controller Link** | - For Pi Zero 2 W: **Micro-USB male to USB-A female OTG adapter** + short USB cable.<br>- For Pi 3B+ / 4: Short standard **USB-A to USB cable** connecting to the power board. | Connects the Raspberry Pi to the ATmega2560 power controller board. | ~**3 € – 5 €** |
| **High Endurance MicroSD Card** | **Kingston High Endurance** (32 GB, `SDCE/32GB`) or **Kingston Industrial** (`SDCIT2/32GB`) / **SanDisk MAX Endurance** | System storage. High/Industrial endurance cards are built to withstand continuous writes and extreme operating temperatures (-25°C to +85°C) in outdoor wallboxes. *Do not compromise on flash quality.* | ~**25 € – 55 €** |

> [!NOTE]
> ### 🔌 12V Auxiliary Power Connector Identification & Pinout
> - **Arrangement:** 2-pin single row (**1x2**) with retention clip / positive lock (pitch 4.20 mm).
> - **Polarity / Wiring:**
>   - **Red wire:** **+12V DC** (positive from the station's internal Mean Well power supply)
>   - **Black wire:** **GND / 0V** (ground return)
> - **Important Distinction:** Inside the charging station, an adjacent harness with identical connectors powers other auxiliary modules (such as the optional RFID reader 0 590 59 or TIC/Bluetooth board). Verify that you tap the 2-pin harness with Red/Black wires delivering 12V DC.
> - **Search references:** *Molex Mini-Fit Jr 2 pin 1x2* / *Molex 5557 2P 4.2mm* / *Molex 39-01-4020*. Pre-crimped 2-pin pigtail cables (*"câble Molex 2 broches 4.2mm pré-câblé"*) make connection to the DC-DC buck converter quick and easy without dedicated crimping pliers.

> [!TIP]
> ### 💡 Pro-Tip: The "PCIe 6+2 Pin" Zero-Crimping Lifehack
> If you do not have dedicated Molex crimping pliers, buy an inexpensive **PC GPU power extension cable (PCIe 6+2 pin)** (~5 € on Amazon) or use an unused modular PSU cable from your PC parts drawer.
> 
> The detachable **2-pin segment** of a PC PCIe power connector uses the **exact same Molex Mini-Fit Jr. 4.2 mm footprint and retention clip** as Legrand's auxiliary power connector!
> 1. Snip the 2-pin segment off the PCIe cable to get a ready-to-use female connector with thick, factory-crimped 18 AWG wires.
> 2. Strip the wire ends and insert them into two **WAGO 221** lever connectors (one for +12V, one for GND).
> 3. Connect the input wires of the buck converter into the other side of the WAGO connectors.
> 4. Use heat shrink tubing or electrical tape over exposed splices for clean wire management and vibration safety.

---

### D. Mechanical Mounting & Fasteners

Field testing on a real Green'up Premium wallbox confirms that **a standard Raspberry Pi 3B+ in an off-the-shelf plastic protective case fits directly into the upper left section of the lateral side panel (the exact location of the official kit)**:

| Component | Status & Specifications | Notes | Indicative Cost |
| :--- | :--- | :--- | :--- |
| **Pragmatic Mounting (Field-Validated)** | **Heavy-duty double-sided foam tape + electrical tape** | The Raspberry Pi in its standard vented case is secured against the smooth internal plastic chassis. No chassis drilling or 3D printer required. Highly stable, zero vibration inside the garage/outdoor box. | ~**1 €** |
| **3D Mounting Bracket (Optional)** | Custom 3D-printed carrier plate | For an OEM-style finish. When designed, must be printed in **PETG** or **ABS/ASA** (avoid PLA due to heat in summer). Secures to the 4 factory M4 bosses. | ~**2 € – 3 €** *(filament)* |

---

## 2. Indicative Budget Summary

| Item | Official Legrand Kit (Ref 059056) | DIY Alternative Kit (Pi Zero 2 W) | DIY Alternative Kit (Pi 3B+) | Field-Tested Kit V1 (Existing Pi 3B+) |
| :--- | :--- | :--- | :--- | :--- |
| **SBC / Processor** | Proprietary Raspberry Pi 3B | Raspberry Pi Zero 2 W (~22 €) | Raspberry Pi 3 Model B+ (~46 €) | Already owned (0 €) |
| **5V Power Supply** | Proprietary Legrand PCB | Waterproof Buck 12V->5V 3A (~8 €) | Waterproof Buck 12V->5V 3A (~8 €) | Fastsaw 12V->5V 3A (~8 €) |
| **Molex Harness & Splicing** | Included in kit | PCIe 6+2 pin cable + 2x WAGO 221 (~6.50 €) | PCIe 6+2 pin cable + 2x WAGO 221 (~6.50 €) | PCIe 6+2 pin cable (5 €) + 2x WAGO 221 (~1.50 €) |
| **USB Link Cable** | Included in kit | OTG adapter + USB cable (~5 €) | Standard USB cable (~3 €) | Standard USB cable (0 € / already owned) |
| **MicroSD Card (Endurance)** | 16 GB standard card | 32 GB High Endurance Kingston (~25 € – 50 €) | 32 GB High Endurance Kingston (~25 € – 50 €) | Already owned (0 €) |
| **Mounting & Insulation** | Stamped metal bracket + screws | Double-sided tape / electrical tape (~1 €) | Double-sided tape / electrical tape (~1 €) | Double-sided tape + electrical tape (~1 €) |
| **TOTAL** | **~500 € to 700 €** | **~67 € – 92 €** *(with quality endurance SD)* | **~89 € – 114 €** *(with quality endurance SD)* | **~15.50 € !** 🎯 *(Power conversion adaptation)* |

---

## 3. Synoptic Wiring Diagrams

### Setup A: Internal Power Tap (Field-Validated Kit V1)

```text
[ Internal Station Power Supply (12V DC) ]
                        |
                        | (Factory Legrand auxiliary harness)
                        v
     [ Molex 2-pin Female Connector (PCIe 6+2 hack) ]
            |                               |
            | (+12V DC - Red)               | (GND / 0V - Black)
            v                               v
    [ WAGO 221 Lever Nut ]          [ WAGO 221 Lever Nut ]
            |                               |
            +---------------+       +-------+
                            |       |
                            v       v
      [ Waterproof DC-DC Buck Converter (12V->5V 3A / 15W) ]
                            |
                            | (Regulated 5.0V via Micro-USB / USB-C)
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
