# Green'Up Link

[🇫🇷 Lire en Français](README.fr.md)

> **Disclaimer :** Green'Up Link is an independent, community-developed project and is not affiliated with, sponsored or endorsed by Legrand. "Green'Up" and "Legrand" are trademarks of their respective owners.

## 📖 Project Overview

**Green'Up Link** is a lightweight, high-performance, independent software written in Rust. It installs directly on the official Legrand Communication Kit (reference 059056) to replace the software ("KitCom") provided by the manufacturer. 

It communicates with the charging station's power board via the internal USB serial link and exposes:
- A clean, responsive **local Web GUI**.
- A comprehensive and documented **REST API**, perfect for home automation integration (Home Assistant, Jeedom, etc.).
- Full compatibility with **EVCC** for smart solar charging (see the [EVCC Integration Guide](docs/EVCC_INTEGRATION.md)).

It also provides an EVerest integration without API/UI via a direct MQTT bridge, allowing the station to be controlled as a standard EVerest hardware module. *(Note: For a cleaner, native EVerest module implementation that replaces EvseManager entirely, check out the excellent community project [evorada/everest-greenup](https://github.com/evorada/everest-greenup) by suda).*

## 🤔 Why this project?

The software provided with the official Communication Kit is bloated. It is buggy, unresponsive, and very difficult to use for standard home automation. 

Moreover, it relies on a Raspberry Pi 3 running an obsolete operating system (Raspbian 9 "Stretch"), which is highly vulnerable from a security standpoint. This project allows you to take back full control of your hardware in a much healthier and more responsive way.

## ⚠️ Current Limitations

This project is actively in development. Features will be added progressively. For now:
- Only manages **direct charging**.
- Only manages the **058001** charging station reference (1 Type 2 side, no Shuko domestic socket, 7kW). *This is the model I own and on which I could test.*
- **DOES NOT MANAGE** the RFID badge reader.
- **DOES NOT MANAGE** schedules (time programming).
- **DOES NOT MANAGE** Modbus communication.
- **DOES NOT MANAGE** the OCPP protocol.

## ✅ Compatibility

- **Operating System**: Legrand official image "Raspbian GNU/Linux 9 (stretch)" (for now).
- **Power board firmware**:
  - `FirmwareBoardA-V01;18;10.hex` (Tested and validated)
  - `FirmwareBoardA-V01;18;04.hex` (Theoretically compatible)
  - `FirmwareBoardA-V01;17;27.hex` (Theoretically compatible)

---

## 🛠️ Compiling the project (For developers)

The project was developed on Windows 11 using the Windows Subsystem for Linux (WSL2), which allows seamless cross-compilation to the Raspberry Pi's ARM architecture. The steps below assume a Windows environment but can easily be adapted for Linux.

1. **Install WSL2** on Windows (e.g. `wsl --install -d Ubuntu`).
2. **Install Rust** in the WSL environment (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`).
3. **Install Linux dependencies (WSL)** required for cross-compilation. On Debian/Ubuntu, run:
   ```bash
   sudo apt update
   sudo apt install -y gcc-arm-linux-gnueabihf pkg-config
   ```
4. **Install Cargo Zigbuild**: `cargo install cargo-zigbuild` (allows easy cross-compilation to ARM).
5. Run the **`.\build_pi.bat`** script (from PowerShell or Windows Command Prompt). This script invokes WSL and generates the optimized binary for the charging station.

---

## ⚙️ Installation on the charging station (Raspberry Pi)

> **WARNING:** Backup the official Legrand image (clone the SD card) before any manipulation to be able to rollback in case of an issue.

> ⚠️ **Important:** The physical installation of the Raspberry Pi in the station MUST be done with the **power off**.

### 1. Legrand kit initialization
Follow the **[official Legrand documentation](https://www.legrand.fr/pro/catalogue/kit-de-communication-ip-pour-bornes-greenup-premium-pour-vehicule-electrique#scroll-to:product-details--documentation-et-conseils-de-pose)** to initialize the Raspberry Pi on your network.
   
### 2. Get SSH access (SD card modification)
The embedded Raspberry Pi 3 uses a standard (unencrypted) installation, but the SSH port is closed and the default password has been changed by Legrand. You must intervene directly on the SD card:

1. Dismount the station (power off) to retrieve the Raspberry Pi SD card and read it on your computer.
2. **Enable SSH**: Create an empty file named `ssh` (no extension) at the root of the `boot` partition.
3. **Reset password**: The `pi` user exists but its password is unknown.
   - On your computer (under Linux or WSL), generate the hash of a new password with: `mkpasswd -m sha-512`
   - Open the main partition (rootfs) of the SD card and edit the `/etc/shadow` file.
   - Find the line starting with `pi:` (e.g., `pi:<unknown-hash>:18508:0:99999:7:::`) and replace the existing hash with the one you just generated.
4. Put the SD card back into the Raspberry Pi and power on the station.
5. You can now connect via SSH: `ssh pi@<station_ip>` using the password you set. The `pi` user has administrator (sudo) rights.

### 3. Securing SSH access
Once connected to the Raspberry Pi via SSH, it is imperative to secure it.
⚠️ **SECURITY NOTE:** The Raspberry Pi runs on an old OS with many known vulnerabilities. **NEVER** expose it to the internet (no port forwarding on your router).

1. **Immediately change the root password**:
   ```bash
   passwd root
   ```
2. **Secure access via public key exchange**:
   From your PC, send your public key (if you don't have one, generate it with `ssh-keygen`):
   ```bash
   ssh-copy-id pi@192.168.1.xxx
   ```
   > ⚠️ **Warning:** Before disabling password authentication, verify that key-based connection works, otherwise you will lose SSH access and will have to dismount the micro SD card again.

3. Disable password authentication in `/etc/ssh/sshd_config` (`PasswordAuthentication no`), as well as root authentication (`PermitRootLogin no`).

4. **Neutralize Legrand security**: 
   The original software includes an emergency "destruction" script (`DeletAll.sh`) that erases all software if the network MAC addresses do not match Legrand's expectations.
   - Edit the file: `nano ~/Desktop/DeletAll.sh`
   - Add the `exit 0` command on the second line (right after `#!/bin/bash`).
   - Remove execution rights: `chmod -x ~/Desktop/DeletAll.sh`

### 4. Installing Green'Up Link
1. Transfer the compiled binary (`greenup-link`) and the scripts to the station via SCP. From your PC:
   ```bash
   scp target/armv7-unknown-linux-gnueabihf/debug/greenup-link pi@192.168.1.xxx:~/Desktop/rust
   scp scripts/* pi@192.168.1.xxx:~/Desktop/rust
   ```
2. Stop the current Legrand services by running the `stop` script.
3. **(Optional but recommended) Disable auto-start** of the original Legrand software. If you don't do this, the official software will restart on every reboot:
   ```bash
   sudo update-rc.d -f CommunicationArduinoRasp remove
   ```

### 5. Auto-start Green'Up Link at boot
To launch the application automatically, create a Systemd service:
1. Create the service file (requires sudo rights): `sudo nano /etc/systemd/system/greenup-link.service`
2. Paste this configuration:
   ```ini
   [Unit]
   Description=GreenUp Link Service
   After=network.target

   [Service]
   ExecStart=/home/pi/Desktop/rust/greenup-link
   WorkingDirectory=/home/pi/Desktop/rust/
   StandardOutput=inherit
   StandardError=inherit
   Restart=always
   User=root

   [Install]
   WantedBy=multi-user.target
   ```
3. Enable and start the service:
   ```bash
   sudo systemctl enable greenup-link
   sudo systemctl start greenup-link
   ```

### 6. Usage
- **Web GUI**: Simply open the station's IP address on port 8080 from your browser (e.g. `http://192.168.1.xxx:8080`).
- **Tests & CLI**: Use the provided PowerShell script `cli-greenup-link.ps1` from your PC to interact with the API via command line.
- **Home Automation**: Check the [REST API Documentation](docs/GREENUP_LINK_REST_API.md) to interface your home automation controller.
- **EVerest Module**: For integration with the open-source EV charging framework EVerest (via MQTT), please refer to the dedicated [EVerest Integration Documentation](crates/greenup-everest/README.md).

---

## 🚀 Future Features

The roadmap includes (with no guaranteed timeline):
- MQTT integration for Home Automation
- Compatibility with recent Raspberry Pi OS releases
- Custom ATmega firmware with more capabilities (like more telemetry info from TIC signals)
- OCPP communication mode support
- Extending compatibility to other station models in the range:
  - Schuko (domestic socket) compatibility
  - Three-phase equipment support
  - Dual charge points wallbox support

*Note: Feel free to use GitHub "Issues" to suggest or request specific features you might need!*

