# ATmega2560 Power Board Firmware Flashing Guide (Official & Custom)

> [!CAUTION]
> ### ⚠️ PROCEED AT YOUR OWN RISK - HIGH RISK OF PERMANENT BRICKING
>
> Flashing firmware onto the ATmega2560 microcontroller of the Legrand Green'Up Premium charging station is an **advanced, high-risk hardware operation**.
>
> * **Irreversible Damage Risk**: If any interruption, power loss, USB serial disconnect, communication timeout, or incorrect file upload occurs during the flashing process, the charging station **may become completely unusable and permanently bricked (HS)**.
> * **No Rollback Procedure**: **No rollback or unbrick procedure is currently identified, documented, or supported**. The board does not expose a standard ISP programming header or hardware DTR auto-reset circuit without specialized laboratory hardware.
> * **No Warranty or Liability**: This guide, procedures, and accompanying scripts are provided for educational and research purposes only, "AS IS", without warranty of any kind. Neither the authors nor contributors shall be held liable for any damages, hardware failure, fire hazards, or personal injury resulting from following these instructions.
>
> **Do not attempt this procedure unless you fully understand the risks and accept total personal responsibility for the outcome.**

---

## 1. Operating Principle & Bootloader Unlock Mechanism

On Legrand Green'Up Premium charging stations, the DTR control line of the USB-to-Serial interface is **physically disconnected** from the RESET pin of the Microchip/Atmel ATmega2560 microcontroller. This is an industrial safety design to prevent accidental MCU reboots or relay cut-offs during high-current EV charging sessions.

To force the microcontroller into its serial bootloader (**Wiring / STK500v2** protocol at **115,200 baud**), Legrand implemented a software **Challenge-Response** handshake:

1. The supervisor software sends a serial command `UpdateAuth:<CRC16-Modbus>` computed from 5 unique factory hardware identifiers stored on the board (`Date`, `SW`, `SN`, `HW`, `Ref`).
2. The ATmega verifies the checksum, replies with `UpdateCodeOK\r\n`, and sets a persistent volatile flag in SRAM (`RAM[0x0ACC] = 1`).
3. As soon as the flashing utility `avrdude` opens `/dev/ttyUSB0` and transmits its initial synchronization byte (`0x1B`), the ATmega detects the armed flag and immediately triggers a hardware **Watchdog Reset** (`0x0EEC4`).
4. The ATmega reboots instantly into the STK500v2 Wiring bootloader, allowing `avrdude` to take control of programming the on-chip Flash memory.

> [!WARNING]
> **The `UpdateAuth` unlock code is unique to EACH individual station!**
> It is NOT a universal manufacturer backdoor. It is dynamically verified by the microcontroller on every request by cross-referencing its 5 hardware parameters and currently active software version.

---

## 2. Step-by-Step Flashing Procedure

### Step 1: Compute Code and Arm Bootloader (`UpdateAuth`)

While `greenup-link` (or your serial supervisor) is actively communicating on `/dev/ttyUSB0`:

1. **Option A (100% Automated via REST API):**
   ```bash
   python3 scripts/calc_update_auth.py --api http://localhost:8080 --arm
   ```
   *The script automatically queries the station via the local REST API, parses the 5 parameters, computes the CRC16-Modbus code, and sends the `UpdateAuth` unlock command.*

2. **Option B (Manual calculation & curl):**
   Compute your station's unlock code:
   ```bash
   python3 scripts/calc_update_auth.py <DATE> <SW> <SN> <HW> <REF>
   # Example: python3 scripts/calc_update_auth.py 42W22 V01.17.78 015184 V02.01.01 058001
   ```
   Then send the command to the API:
   ```bash
   curl -X POST http://localhost:8080/api/command -d "UpdateAuth:<COMPUTED_CODE>"
   ```

3. **Verification:**
   Confirm in `greenup-link` logs that the board acknowledged the unlock request:
   ```text
   [SERIAL_TX] UpdateAuth:xxxxx
   [SERIAL_RX] UpdateCodeOK
   ```

> [!NOTE]
> The bootloader arming flag `0x0ACC = 1` remains persistent in SRAM as long as the board remains powered on, until the first `avrdude` sync byte arrives.

---

### Step 2: Release the Serial Port for the System

Stop `greenup-link` to release `/dev/ttyUSB0` so that `avrdude` can claim the device:

```bash
# If running in a terminal or tmux session:
pkill -f greenup-link
# (or press Ctrl+C in its terminal)

# If running as a systemd service:
sudo systemctl stop greenup-link
```

---

### Step 3: Flash the Firmware with `avrdude`

Execute `avrdude` using the **wiring** programmer profile at 115,200 baud:

```bash
sudo avrdude -C /etc/avrdude.conf -v -p atmega2560 -c wiring -P /dev/ttyUSB0 -b 115200 -D -U flash:w:/path/to/<FIRMWARE>.hex:i
```

**Expected output:**
```text
avrdude: AVR device initialized and ready to accept instructions
avrdude: Device signature = 0x1e9801 (probably m2560)
avrdude: reading input file "..."
avrdude: writing flash (146xxx bytes):
Writing | ################################################## | 100% 21.12s
avrdude: verifying ...
avrdude: 146xxx bytes of flash verified
avrdude: safemode: Fuses OK (E:FD, H:D8, L:FF)
avrdude: stk500v2_command(): unknown status 0x01
avrdude: stk500v2_disable(): failed to leave programming mode
avrdude done.  Thank you.
```

> [!TIP]
> The trailing message `stk500v2_disable(): failed to leave programming mode` is **completely normal and expected** with the Atmel Wiring bootloader: upon completion, the bootloader directly jumps to the freshly programmed Flash application address without sending an STK500 leave acknowledgment.

---

### Step 4: Restart `greenup-link` and Verify Version

Restart `greenup-link`:
```bash
./target/release/greenup-link
# or: sudo systemctl start greenup-link
```

Verify that the station boots properly and reports its version:
```bash
curl -X POST http://localhost:8080/api/command -d "SoftwareVersion?"
```

---

## 3. Mathematical Specification of the `UpdateAuth` Formula

### 3.1. The 5 Input Parameters

Parameters are queried over the serial port (USART1) using official protocol commands:

| Parameter | Serial Query | Expected Format | Example |
| :--- | :--- | :--- | :--- |
| **`Date`** | `WeekYearProduction?` | 5 ASCII chars: `WWYYY` | `42W22` (Week 42, Year 2022) |
| **`SW`** | `SoftwareVersion?` | 9 normalized chars: `Vxx.yy.zz` | `V01.17.78` or `V01.18.10` |
| **`SN`** | `SerialNumber?` | 6 ASCII digits | `015184` |
| **`HW`** | `HardwareVersion?` | 9 normalized chars: `Vxx.yy.zz` | `V02.01.01` |
| **`Ref`** | `Reference?` | 6 ASCII digits | `058001` |

> [!IMPORTANT]
> The software version `SW` must correspond to **the firmware version currently active on the board when the command is sent**, NOT the target version you intend to flash!

---

### 3.2. 28-Character Interleaving Formula

The Legrand bootloader verification routine constructs an intermediate **28-character ASCII string** by interleaving specific character indices of the 5 parameters:

```text
Position :  0   1   2   3   4   5   6   7   8   9  10  11  12  13  14  15  16  17  18  19  20  21  22  23  24  25  26  27
Source   : Date SW  SN  HW Ref Date SW  SN  HW Ref Date SW  SN  HW Ref Date SW  SN  HW Ref  SW  SN  HW Ref  SW  SN  HW Ref
Index    : [0] [8] [2] [8] [1] [4] [1] [1] [7] [3] [1] [4] [5] [1] [4] [3] [2] [0] [2] [5] [5] [4] [4] [2] [7] [3] [5] [0]
```

**Index mapping table:**

| Block | Key Index | Variable | Character Index | Description |
| :--- | :--- | :--- | :--- | :--- |
| **1** | `0` | `Date` | `[0]` | 1st digit of production week |
| | `1` | `SW` | `[8]` | Last digit of active SW version |
| | `2` | `SN` | `[2]` | 3rd digit of serial number |
| | `3` | `HW` | `[8]` | Last digit of HW version |
| | `4` | `Ref` | `[1]` | 2nd digit of commercial reference |
| **2** | `5` | `Date` | `[4]` | 2nd digit of production year |
| | `6` | `SW` | `[1]` | 1st major version digit |
| | `7` | `SN` | `[1]` | 2nd digit of serial number |
| | `8` | `HW` | `[7]` | Penultimate digit of HW version |
| | `9` | `Ref` | `[3]` | 4th digit of commercial reference |
| **3** | `10` | `Date` | `[1]` | 2nd digit of production week |
| | `11` | `SW` | `[4]` | 1st minor version digit |
| | `12` | `SN` | `[5]` | Last digit of serial number |
| | `13` | `HW` | `[1]` | 1st major digit of HW version |
| | `14` | `Ref` | `[4]` | 5th digit of commercial reference |
| **4** | `15` | `Date` | `[3]` | 1st digit of production year |
| | `16` | `SW` | `[2]` | 2nd major version digit |
| | `17` | `SN` | `[0]` | 1st digit of serial number |
| | `18` | `HW` | `[2]` | 2nd major digit of HW version |
| | `19` | `Ref` | `[5]` | Last digit of commercial reference |
| **5** | `20` | `SW` | `[5]` | 2nd minor version digit |
| | `21` | `SN` | `[4]` | 5th digit of serial number |
| | `22` | `HW` | `[4]` | 1st minor digit of HW version |
| | `23` | `Ref` | `[2]` | 3rd digit of commercial reference |
| **6** | `24` | `SW` | `[7]` | 1st patch/build version digit |
| | `25` | `SN` | `[3]` | 4th digit of serial number |
| | `26` | `HW` | `[5]` | 2nd minor digit of HW version |
| | `27` | `Ref` | `[0]` | 1st digit of commercial reference |

---

### 3.3. CRC-16 Modbus Computation

The resulting 28-character ASCII string is passed to the standard **CRC-16 Modbus** algorithm:
- **Polynomial (reversed / LSB first)**: `0xA001` (equivalent to `0x8005` forward).
- **Initial register value**: `0xFFFF`.
- **Processing order**: Least Significant Bit (LSB) first.
- **Output**: 16-bit unsigned integer (0 to 65535) formatted as a decimal ASCII string.

---

### 3.4. Reference Test Vector (Validation)

To test and validate your calculation or script implementation:

- **Input parameters:**
  - `WeekYearProduction` = `"42W22"`
  - `SoftwareVersion` = `"V01.17.78"`
  - `SerialNumber` = `"015184"`
  - `HardwareVersion` = `"V02.01.01"`
  - `Reference` = `"058001"`
- **Generated 28-char key:**
  `4851520100214002102178087110`
- **Expected CRC-16 Modbus output:**
  `43417`
- **Final serial command:**
  `UpdateAuth:43417`

---

## 4. Standalone Calculator Script: `calc_update_auth.py`

The standalone Python utility is located in [`greenup-link/scripts/calc_update_auth.py`](file:///c:/Users/rapha/Desktop/Maj%20borne%20legrand/retro_ingenierie/legrand/greenup-link/scripts/calc_update_auth.py). It requires **no third-party dependencies** and runs on any standard Python 3 installation.

### Usage examples:

#### 1. Positional Arguments:
```bash
python3 scripts/calc_update_auth.py 42W22 V01.17.78 015184 V02.01.01 058001
```

#### 2. Named Flags:
```bash
python3 scripts/calc_update_auth.py --date 42W22 --sw V01.17.78 --sn 015184 --hw V02.01.01 --ref 058001
```

#### 3. Automatic Mode via `greenup-link` REST API:
```bash
# Query parameters from local API and display computed code:
python3 scripts/calc_update_auth.py --api http://localhost:8080

# Query parameters, compute code, and IMMEDIATELY ARM the bootloader:
python3 scripts/calc_update_auth.py --api http://localhost:8080 --arm
```

#### 4. Interactive Mode:
Run without arguments to be prompted for the 5 parameters:
```bash
python3 scripts/calc_update_auth.py
```

---

## 5. Firmware Patcher Script: `patch_firmware_17_77.py`

To reproducibly patch an official Legrand 17.77 firmware file with the real-time Linky TIC telemetry handler, use [`scripts/patch_firmware_17_77.py`](file:///c:/Users/rapha/Desktop/Maj%20borne%20legrand/retro_ingenierie/legrand/greenup-link/scripts/patch_firmware_17_77.py):

```bash
python3 scripts/patch_firmware_17_77.py -i hex/FirmwareBoardA-V01_17_77.hex -o hex/FirmwareBoardA-V01_17_77c.hex
```

This utility performs strict SHA-256 pre-validation against the official Legrand 17.77 binary payload (`d46a6bd6...`), applies the surgical bytecode patch, and verifies the output file integrity (`1fca0124...`). It aborts safely if the input file does not match the official firmware.
