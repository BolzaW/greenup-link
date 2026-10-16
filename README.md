# Green'Up Link

[🇫🇷 Lire en Français](README.fr.md)

> **Disclaimer :** Green'Up Link is an independent, community-developed project and is not affiliated with, sponsored or endorsed by Legrand. "Green'Up" and "Legrand" are trademarks of their respective owners.

## 📖 Project Overview

**Green'Up Link** is a lightweight, high-performance, independent software written in Rust. It installs directly on the official Legrand Communication Kit (reference 059056) to replace the software ("KitCom") provided by the manufacturer. 

It communicates with the charging station's power board via the internal serial link and exposes:
- A clean, responsive **local Web GUI** with live visualizer, charging controls (7A–32A), Linky TIC status, and an integrated real-time debug console (SSE log streaming).
- A comprehensive and documented **REST API**, perfect for home automation integration (Home Assistant, Jeedom, Node-RED, etc.).
- Complete **log management** with daily rolling files (`greenup_YYYY-MM-DD.log`), 7-day automatic retention, and hot-reloadable log levels (`TRACE`, `DEBUG`, `INFO`, `WARN`, `ERROR`).
- Full compatibility with **EVCC** for smart solar charging (see the [EVCC Integration Guide](docs/EVCC_INTEGRATION.md)).

It also provides an EVerest integration without API/UI via a direct MQTT bridge, allowing the station to be controlled as a standard EVerest hardware module. *(Note: For a cleaner, native EVerest module implementation that replaces EvseManager entirely, check out the excellent community project [evorada/everest-greenup](https://github.com/evorada/everest-greenup) by suda).*

## 🤔 Why this project?

The software provided with the official Communication Kit is bloated. It is buggy, unresponsive, and very difficult to use for standard home automation. 

Moreover, it relies on a Raspberry Pi 3 running an obsolete operating system (Raspbian 9 "Stretch"), which is highly vulnerable from a security standpoint. This project allows you to take back full control of your hardware in a much healthier and more responsive way.

## ⚠️ Current Limitations

This project is actively in development. Features will be added progressively. For now:
- Only manages **direct charging**.
- Only manages the **058001 / 059001** charging station reference (1 Type 2 side, no Shuko domestic socket, 7kW). *This is the model I own and on which I could test.*
- **DOES NOT MANAGE** the RFID badge reader.
- **DOES NOT MANAGE** schedules (time programming).
- **DOES NOT MANAGE** Modbus communication.
- **DOES NOT MANAGE** the OCPP protocol.

## ✅ Compatibility

- **Operating System**:
  - Legrand official image "Raspbian GNU/Linux 9 (stretch)" (32-bit ARMv7).
  - Modern Raspberry Pi OS (32-bit `armv7` and 64-bit `aarch64`).
- **Power board firmware**:
  - `FirmwareBoardA-V01;18;10.hex` (Tested and validated)
  - `FirmwareBoardA-V01;18;04.hex` (Theoretically compatible)
  - `FirmwareBoardA-V01;17;27.hex` (Theoretically compatible)

---

## 🛠️ Compiling the project (For developers)

The project is structured as a Cargo workspace and supports cross-compilation for both legacy 32-bit ARM (Debian Stretch) and modern 64-bit ARM (Raspberry Pi OS 64-bit).

Ready-to-use compilation scripts are available in the [`builds/`](builds/) directory:

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
For full environment setup details, see the [64-bit Compilation Guide](docs/ENV_COMPILATION_AARCH64.md).

---

## ⚙️ Installation on the charging station (Raspberry Pi)

> **WARNING:** Always backup your official Legrand SD card image before making any changes so that you can revert if needed.

> ⚠️ **Important:** Physical manipulation of the Raspberry Pi inside the wallbox MUST be performed with the **mains power completely turned off**.

### 1. Legrand kit initialization
Follow the **[official Legrand documentation](https://www.legrand.fr/pro/catalogue/kit-de-communication-ip-pour-bornes-greenup-premium-pour-vehicule-electrique#scroll-to:product-details--documentation-et-conseils-de-pose)** to initialize the Raspberry Pi on your local network.
   
### 2. Get SSH access (SD card modification)
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

### 4. Automated Installation

#### Option A: Quick 1-Liner (Automatic architecture detection & download)
If the Raspberry Pi has internet access, simply run the installation script directly:
```bash
ssh pi@<station_ip>
curl -sSL https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/install.sh | sudo bash
```
> [!TIP]
> The script automatically identifies the system architecture (`armv7` 32-bit legacy or `aarch64` 64-bit), queries the latest release on GitHub, downloads and extracts the matching binary into `/home/pi/greenup/`.

To update an existing installation to the latest GitHub release, re-run with `--update`:
```bash
curl -sSL https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/install.sh | sudo bash -s -- --update
```

#### Option B: Offline / Manual Transfer (SCP)
1. **Transfer the binary and installer** to the Raspberry Pi from your PC via SCP:
   ```bash
   # Using pre-compiled release files:
   scp greenup-link scripts/install.sh pi@<station_ip>:~/
   ```
   *(Or if you compiled from source, use `target/armv7-unknown-linux-gnueabihf/release/greenup-link` or `target/aarch64-unknown-linux-gnu/release/greenup-link`).*

2. **Connect via SSH and execute the installer**:
   ```bash
   ssh pi@<station_ip>
   chmod +x install.sh
   sudo ./install.sh
   ```

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
- **EVerest Module**: Refer to the [EVerest Integration Documentation](crates/greenup-everest/README.md) for native MQTT integration.

---

## 🚀 Future Features

The roadmap includes (with no guaranteed timeline):
- MQTT integration for Home Automation
- Compatibility with recent Raspberry Pi OS releases
- Custom ATmega firmware with more capabilities
- OCPP communication mode support
- Extending compatibility to other station models in the range:
  - Schuko (domestic socket) compatibility
  - Three-phase equipment support
  - Dual charge points wallbox support

*Note: Feel free to use GitHub "Issues" to suggest or request specific features you might need!*
