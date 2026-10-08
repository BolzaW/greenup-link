# Flashing the ATmega Firmware

> [!CAUTION]
> **EXPERIMENTAL & HIGH RISK PROCEDURE**
> This procedure allows flashing an arbitrary firmware (Intel HEX) to the power board (ATmega2560) bypassing Legrand's official version checks.
> - Flashing an incompatible firmware can **brick** your charging station.
> - An interruption during the flash process might corrupt the bootloader.
> - You can flash older firmware (downgrade) or completely custom unverified firmware.
> - Proceed entirely at your own risk.

## Context
The power board of the Green'Up Premium station is controlled by an ATmega2560 micro-controller. Updates are normally pushed through Legrand's locked ecosystem. Through reverse engineering, we identified the procedure used by the internal java daemon to trigger a firmware update, relying on a specific serial command and standard Arduino tools (`avrdude`).

## Flashing Scenario

To flash the ATmega, the following sequence of commands and events must be rigorously executed:

### 1. Trigger Bootloader Mode
Send the `Z` command over the serial interface.
```text
TX: Z\r
```
This specific command is a software reset trigger. When received, the ATmega reboots immediately and drops into its serial bootloader, waiting for a new firmware.

### 2. Wait for Bootloader Initialization
Wait for **~1.5 seconds**.
This delay is strictly necessary to let the micro-controller reboot and the bootloader to fully initialize its serial listening interface.

### 3. Run AVRDude
Launch `avrdude` to upload the new Intel HEX firmware file. The bootloader uses the standard `wiring` protocol over `115200` baud.

```bash
sudo avrdude -C /etc/avrdude.conf -v -p atmega2560 -c wiring -P /dev/ttyUSB0 -b 115200 -D -U flash:w:/path/to/firmware.hex:i
```
*(Note: Replace `/dev/ttyUSB0` with the correct serial port of the ATmega).*

Wait for the process to complete successfully.

### 4. Wait for ATmega Application Boot
Wait for **~2.0 seconds** after `avrdude` successfully exits.
This gives the ATmega time to boot the newly flashed firmware and initialize its internal state machine.

### 5. Resume Serial Control
The newly booted ATmega will typically broadcast a `RaspberryPi?` ping to see if the supervision board is alive.
Send the initialization handshake to resume normal control:
```text
TX: RaspberryPiModeOK\r
```
The ATmega should respond by acknowledging its active side (e.g. `Side:1`), indicating the flash was successful and the station is ready for operation.
