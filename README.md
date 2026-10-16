# Green'Up Link

[🇫🇷 Lire en Français](README.fr.md)

> [!CAUTION]
> ### ⚡ Critical Electrical Safety Warning & Disclaimer of Liability
> 
> **Mains Voltage Danger (230V / 400V):**
> Fatal electrical currents are present inside the charging station. **All physical interventions, opening of the station enclosure, hardware wiring, or modifications MUST strictly be performed with the main electrical supply completely turned OFF (mains circuit breaker switched off).** Always test and verify the complete absence of voltage before opening the unit. If you are not a qualified electrician, do not open the station.
> 
> **Disclaimer of Liability & "AS IS" Software:**
> - **Green'Up Link** is an independent community project provided on an **"AS IS" BASIS**, without warranties or conditions of any kind, whether express, implied, or statutory (including fitness for a particular purpose, safety, or reliability).
> - The author, developers, and contributors shall **under no circumstances be held liable** for any direct, indirect, incidental, special, exemplary, or consequential damages whatsoever (including, but not limited to, electrical shock, electrocution, personal injury, death, fire, electrical panel damage, vehicle or battery degradation, property damage, loss of manufacturer warranty, or data loss) resulting from the use, installation, modification, or inability to use this software, hardware, or documentation.
> - **The end user assumes total and sole responsibility for using this software and modifying any hardware.**
> 
> **Trademark Notice:** Green'Up Link is an independent, community-developed project and is not affiliated with, sponsored, or endorsed by Legrand. "Green'Up" and "Legrand" are registered trademarks of their respective owners.

## 📖 Project Overview

**Green'Up Link** is a lightweight, high-performance, independent software written in Rust. It can be installed directly on the official Legrand Communication Kit (reference 059056) to replace the manufacturer's software ("KitCom"), or on a **standard Raspberry Pi** (Pi Zero 2 W, Pi 3, Pi 4, etc.) running an official Raspberry Pi OS 64-bit Lite image connected via a USB cable (see the [DIY Communication Kit Guide](docs/KIT_COMMUNICATION_DIY.md)). 

It communicates with the charging station's power board via an internal USB cable and exposes:
- A clean, responsive **local Web GUI** with live visualizer, charging controls (7A–32A), Linky TIC status, and an integrated real-time debug console (SSE log streaming).
- A comprehensive and documented **REST API**, perfect for home automation integration (Home Assistant, Jeedom, Node-RED, etc.).
- Complete **log management** with daily rolling files (`greenup_YYYY-MM-DD.log`), 7-day automatic retention, and hot-reloadable log levels (`TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`).
- Full compatibility with **EVCC** for smart solar charging (see the [EVCC Integration Guide](docs/EVCC_INTEGRATION.md)).

*(Experimental / PoC)*: A standalone `greenup-everest` module is included as an experimental proof of concept to interface with the open-source EVerest framework via MQTT (not the core focus of this project; see also [evorada/everest-greenup](https://github.com/evorada/everest-greenup)).

## 🤔 Why this project?

The software provided with the official Communication Kit is bloated, buggy, unresponsive, and very difficult to integrate with modern home automation systems.

Moreover, the official Legrand software stack (running outdated versions of Apache Tomcat 9, an unmaintained Java runtime, and obsolete system components on Raspbian 9 "Stretch") contains **multiple known CVEs and critical vulnerabilities**. Exposing this official stack directly to the internet (or even on an untrusted local network) poses severe security risks. Green'Up Link eliminates this legacy stack entirely in favor of a minimal, memory-safe Rust binary with no external runtime dependencies, giving you back full control over your charging station.

## ⚠️ Current Limitations

This project is actively in development:
- **Scope of the software**: Green'Up Link only replaces the communication kit (software layer running on the Raspberry Pi). The charging station retains all of its native intelligence, electrical protections, and autonomous operation via its internal ATmega2560 power board.
- **Tested hardware**: I can only test and validate on my own hardware: reference **058001** (single Type 2 socket, 7.4 kW single-phase, without domestic socket).
- **Type 2 compatibility**: All Green'up Premium stations should nominally work with their Type 2 socket.
- **Domestic socket (Schuko) & Three-phase**: Not supported at this stage due to a lack of knowledge, captured serial communication logs, and test hardware to investigate these configurations.
- **Direct charging only**: No support for schedules / onboard time programming (use Home Assistant or EVCC to schedule charges instead).
- **Does NOT manage**: RFID badge reader, Modbus communication, or OCPP protocol.

## ✅ Compatibility

- **Operating System**:
  - Legrand official legacy image: "Raspbian GNU/Linux 9 (stretch)" 32-bit (`armv7`, glibc 2.24). *(Note: modern 32-bit Debian/Raspberry Pi OS is not supported — only the 32-bit legacy Stretch build is provided).*
  - Modern Raspberry Pi OS: 64-bit (`aarch64`, Bullseye / Bookworm) Lite or Desktop.
- **Station hardware models**: Refer to the comprehensive [Hardware Compatibility Matrix](docs/COMPATIBLE_HARDWARE.md) for the complete reference list and support status (Full, Partial, or None).
- **Power board firmware**:
  - `FirmwareBoardA-V01;18;10.hex` (Tested and validated)
  - `FirmwareBoardA-V01;17;27.hex` (Tested and validated)
  - `FirmwareBoardA-V01;18;04.hex` (Theoretically compatible)

> [!NOTE]
> **Regarding Legrand system images:** It is possible that newer Legrand factory image revisions exist whose exact specifications are not currently known. Legrand does not provide OS migration mechanisms between major image releases — only the application software stack and a few Debian packages are updated via Legrand's update mechanism. As a result, certain Legrand image versions might have glibc or kernel differences incompatible with the current prebuilt legacy binary. If you encounter such an image, please open an issue with your system details (`uname -a`, `ldd --version`), and a dedicated compilation toolchain can be created.

---

## 🛠️ Compiling the project (For developers)

The project is structured as a Cargo workspace and supports cross-compilation for both legacy 32-bit ARM (Debian Stretch) and modern 64-bit ARM (Raspberry Pi OS 64-bit).

Ready-to-use compilation scripts are available in the [`builds/`](builds/) directory. **Note:** These batch scripts are designed to be run from Windows using **WSL (Ubuntu)** with the cross-compilation toolchains installed.

| Script | Target Architecture | Build Type | Output Binary / Package |
| :--- | :--- | :--- | :--- |
| **`builds/release-legacy.bat`** | 32-bit ARM (`armv7`) | Release (glibc 2.24) | `target/armv7-unknown-linux-gnueabihf/release/greenup-link` + `.zip` |
| **`builds/release-aarch64.bat`** | 64-bit ARM (`aarch64`) | Release (Standard) | `target/aarch64-unknown-linux-gnu/release/greenup-link` + `.zip` |

For manual compilation in WSL or Linux:
```bash
# For modern 64-bit Raspberry Pi OS (Pi 3 / 4 / 5):
cargo build --release -p greenup-link --target aarch64-unknown-linux-gnu

# For legacy 32-bit Legrand image (glibc 2.24 / Debian Stretch):
cargo zigbuild --release -p greenup-link --target armv7-unknown-linux-gnueabihf.2.24
```
For full environment setup details, see the [Compilation Environment Guide](docs/ENV_COMPILATION_AARCH64_ARM7.md).

---

## ⚙️ Installation on the charging station (Raspberry Pi)

> **WARNING:** Always backup your official Legrand SD card image before making any changes so that you can revert if needed.

> ⚠️ **Important:** Physical manipulation of the Raspberry Pi inside the wallbox MUST be performed with the **mains power completely turned off**.

### 1. Legrand kit initialization
Follow the **[official Legrand documentation](https://www.legrand.fr/pro/catalogue/kit-de-communication-ip-pour-bornes-greenup-premium-pour-vehicule-electrique#scroll-to:product-details--documentation-et-conseils-de-pose)** to initialize the Raspberry Pi on your local network.

*(If you do not own the official Legrand kit, see the [DIY Communication Kit Guide](docs/KIT_COMMUNICATION_DIY.md) to build an alternative communication hardware setup with a standard Raspberry Pi).*
   
### 2. Get SSH access (SD card modification)

> [!NOTE]
> **Work in progress:** Tools and automated procedures are currently being developed to enable SSH access directly over the network without requiring physical disassembly or SD card removal.

The embedded Raspberry Pi 3 uses a standard installation, but SSH is closed and the default password was changed by Legrand:

1. Turn off power and retrieve the microSD card from the Raspberry Pi.
2. **Enable SSH**: Create an empty file named `ssh` (no extension) at the root of the `boot` partition.
3. **Reset password**:
   - On your PC (Linux or WSL), generate a password hash: `mkpasswd -m sha-512`
   - Open the main `rootfs` partition and edit `/etc/shadow`.
   - Replace the hash on the line starting with `pi:` with your generated hash.
4. Insert the SD card back into the Raspberry Pi and power on the station.
5. Connect via SSH: `ssh pi@<station_ip>` using your password (`pi` has sudo rights).

### 3. Securing SSH access
⚠️ **SECURITY NOTE:** The Raspberry Pi runs on an older OS. **NEVER** expose it directly to the public internet (no port forwarding on your router).

1. **Change the root password**:
   ```bash
   sudo passwd root
   ```
2. **Install your SSH public key**:
   ```bash
   # From your PC:
   ssh-copy-id pi@<station_ip>
   ```
3. Disable password authentication in `/etc/ssh/sshd_config` (`PasswordAuthentication no`, `PermitRootLogin no`), then reload ssh: `sudo systemctl reload ssh`.

### 4. Installation of Green'Up Link

#### GitHub Releases & Supported Architectures
Pre-compiled packages and standalone binaries are available on the [GitHub Releases page](https://github.com/BolzaW/greenup-link/releases):

| Release Asset | Architecture / Target | Intended System | Package Contents |
| :--- | :--- | :--- | :--- |
| **`greenup-link-vX.Y.Z-armv7-legacy.zip`** | ARMv7 32-bit (`armv7-unknown-linux-gnueabihf.2.24`) | **Official Legrand kit** (Debian 9 Stretch, glibc 2.24) and 32-bit Raspberry Pi OS | `greenup-link`, `install.sh`, `README.md`, `README.fr.md` |
| **`greenup-link-vX.Y.Z-aarch64.zip`** | AArch64 64-bit (`aarch64-unknown-linux-gnu`) | **Standard Raspberry Pi OS 64-bit Lite** (Pi Zero 2 W, Pi 3, Pi 4, Pi 5) | `greenup-link`, `install.sh`, `README.md`, `README.fr.md` |
| **`greenup-link-armv7-legacy`** | ARMv7 32-bit | Standalone executable (glibc 2.24 compatible) | Single binary |
| **`greenup-link-aarch64`** | AArch64 64-bit | Standalone executable | Single binary |
| **`SHA256SUMS.txt`** | - | SHA256 checksums for integrity verification | Text file |

#### Option A: Quick 1-Liner (Automatic architecture detection & download)
If the Raspberry Pi has internet access, simply run the installation script directly:
```bash
ssh pi@<station_ip>
curl -sSL https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/install.sh | sudo bash
```
> [!TIP]
> The script automatically identifies the system architecture (`armv7` 32-bit legacy or `aarch64` 64-bit), queries the latest release on GitHub, downloads the matching standalone binary directly, and installs `greenup-link` into `/home/pi/greenup/`.

To update an existing installation to the latest GitHub release, re-run with `--update`:
```bash
curl -sSL https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/install.sh | sudo bash -s -- --update
```

#### Option B: Offline / Manual Installation (Using Release ZIP)
If your charging station does not have direct internet access:
1. **Download the appropriate ZIP archive** from the [GitHub Releases page](https://github.com/BolzaW/greenup-link/releases) on your PC:
   - For official Legrand kit: `greenup-link-vX.Y.Z-armv7-legacy.zip`
   - For modern 64-bit Raspberry Pi OS: `greenup-link-vX.Y.Z-aarch64.zip`
2. **Transfer the archive to the Raspberry Pi** via SCP:
   ```bash
   scp greenup-link-v*.zip pi@<station_ip>:~/
   ```
3. **Connect via SSH, extract the archive, and run the installer**:
   ```bash
   ssh pi@<station_ip>
   unzip greenup-link-v*.zip
   chmod +x install.sh
   sudo ./install.sh
   ```
   *(Note: The archive already bundles `install.sh` and the pre-configured `greenup-link` binary. If `unzip` is not installed on the station, extract the archive on your PC and transfer the contents).*

---

The automated installer performs all required setup steps:
- Installs `greenup-link` into `/home/pi/greenup/` with executable permissions (auto-downloaded from GitHub or moved from local directory).
- Stops and disables legacy Legrand services (Apache Tomcat 9 and Java daemon).
- Neutralizes the Legrand init.d startup script (`/etc/init.d/CommunicationArduinoRasp`) with an `exit 0` safeguard (creating a `.old` backup).
- Neutralizes Legrand's emergency self-deletion script (`DeletAll.sh`) with an `exit 0` safeguard and revokes execution rights (creating a `.old` backup).
- Generates and installs the Systemd service unit `/etc/systemd/system/greenup-link.service`.
- Enables and starts the `greenup-link` service immediately.

### 5. Managing the Service

- **Check service status**:
  ```bash
  sudo systemctl status greenup-link
  ```
- **Inspect live logs in console**:
  ```bash
  sudo journalctl -u greenup-link -f
  ```
- **Restart the service**:
  ```bash
  sudo systemctl restart greenup-link
  ```
- **Stop the service**:
  ```bash
  sudo systemctl stop greenup-link
  ```

### 6. Usage
- **Web GUI**: Open `http://<station_ip>:8080` in your browser. Includes interactive telemetry, charging controls (7A–32A), Linky TIC status, and real-time live console with SSE log streaming.
- **REST API**: Refer to the complete [REST API Documentation](docs/GREENUP_LINK_REST_API.md) to integrate with Home Assistant, Jeedom, Node-RED, etc.
- **Interactive CLI**: Use `scripts/cli-greenup-link.ps1` from PowerShell on Windows to test endpoints or send raw serial commands.
- **EVCC Integration**: Refer to the [EVCC Integration Guide](docs/EVCC_INTEGRATION.md) for smart solar diversion charging.

---

## 🚀 Future Features

The roadmap includes (with no guaranteed timeline):
- **Native MQTT integration** with **Home Assistant Auto-Discovery** (automatic entity creation for sensors, switches, and charging current control).
- **Charging session history**: Recording charging sessions in CSV format, with session visualization and CSV export directly from the Web GUI.
- Custom ATmega firmware with enhanced telemetry and features.
- Support for the OCPP communication protocol.
- Extending compatibility to other station models in the range (domestic Schuko socket, three-phase equipment, dual charge points).

---

## 🤝 Call for Testers & Contributions

I am actively seeking testers who own different hardware configurations of the Legrand Green'up Premium range (e.g. wallboxes equipped with a domestic Schuko socket, three-phase models, dual charge points, or different PCB / firmware revisions).

The goal is to collect serial communication logs to understand how these variants behave and extend Green'Up Link to support the entire range.

If you have one of these models and would like to help test or share logs, please feel free to:
- Open a **[GitHub Issue](https://github.com/BolzaW/greenup-link/issues)**
- Or reach out directly by email: **[bolzaw.dev@gmail.com](mailto:bolzaw.dev@gmail.com)**
