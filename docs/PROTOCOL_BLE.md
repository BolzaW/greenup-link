# Legrand Bluetooth Low Energy (BLE) Protocol

> [!NOTE]
> **Green'up Link Architecture & Scope:**  
> Green'up Link connects directly to the charging station's physical UART interface (`USART1`, via internal USB) rather than using Bluetooth Low Energy. The physical serial bus is faster, supports unsolicited asynchronous push events (real-time telemetry and state changes), and provides full bidirectional supervision without wireless range or pairing constraints.  
> This document describes the functional supervision, telemetry, and control subset of the binary BLE protocol for informational purposes and community projects targeting wireless monitoring.  
> *(Low-level hardware recovery, STK500v2 OTA bootloader flashing, and factory calibration routines are documented separately in the `greenup-tools` repository).*

---

## 1. Overview & Architecture

Legrand Green'up Premium charging stations incorporate an internal **u-blox NINA-B112** transceiver (Bluetooth Low Energy 4.2 / 5.0) wired to the ATmega2560 mainboard via an internal serial link operating at **115200 bauds, 8 data bits, no parity, 1 stop bit (8N1)**.

### Communication Paradigm
Unlike the supervisory serial port (`USART1`), which pushes unsolicited asynchronous text events (`State:B`, `CCI:16.02`, `CC:24`), the Bluetooth interface is **strictly passive**:
- **Half-Duplex Master-Slave (Polling / Pull):** The station never sends unsolicited data over Bluetooth. It only responds when queried by an authorized client (such as the official Legrand mobile application).
- **Industrial Integrity Guard (CRC-16):** Every packet (both requests and responses) is protected by a standard **CRC-16-IBM / Modbus** checksum. Packets with invalid checksums or malformed framing are discarded immediately.
- **State Convergence:** Commands received over Bluetooth update the exact same underlying finite-state machine and current setpoint registers as commands received over the physical serial bus.

---

## 2. BLE GATT Interface & Services

The u-blox NINA transceiver operates in **Serial Port Service (SPS)** Data Mode and exposes the following standard GATT services:

### 2.1 u-blox Serial Port Service (SPS)
- **Service UUID:** `2456e1b9-26e2-8f83-e744-f34f01e9d701`
- **FIFO Data Characteristic:** `2456e1b9-26e2-8f83-e744-f34f01e9d703`
  - **Properties:** `WRITE WITHOUT RESPONSE`, `WRITE`, `NOTIFY`
  - **Downstream (Client $\rightarrow$ Station):** Commands are sent by writing binary frames directly to this characteristic using **Write Without Response** (`response=False`).
  - **Upstream (Station $\rightarrow$ Client):** Responses from the station are returned asynchronously as **GATT Notifications**.
  - **Requirement:** The central client **must subscribe to notifications** on this characteristic before transmitting any request frames.
- **Credits Characteristic:** `2456e1b9-26e2-8f83-e744-f34f01e9d704`
  - By default, the module operates in **Connection Without Credits (Transparent Mode)**. Data flows without software flow control overhead.

### 2.2 Standard Device Information Service
- **Service UUID:** `0000180a-0000-1000-8000-00805f9b34fb`
  - Model Number (`00002a24-...`): `NINA-B112`
  - Manufacturer (`00002a29-...`): `u-blox`

---

## 3. Packet Framing & Checksum Specification

Every binary frame follows a structured multi-byte envelope:

```text
+--------+--------+----------------+--------------------------+--------+----------+----------+
| Byte 0 | Byte 1 |     Byte 2     | Bytes 3 .. (Len - 4)     | Byte L-2| Byte L-1 |  Byte L  |
|  SOH   |  Type  | Opcode/Subcmd  | Payload Data (0..N bytes)|  EOP   |  CRC_HI  |  CRC_LO  |
| (0x01) | (1 B)  |     (1 B)      |         (Variable)       | (0xFF) |  (1 B)   |  (1 B)   |
+--------+--------+----------------+--------------------------+--------+----------+----------+
```

### 3.1 Field Definitions

| Field | Length | Value | Description |
| :--- | :---: | :---: | :--- |
| **SOH** | 1 byte | `0x01` | **Start of Header**: All valid frames must begin with `0x01`. Frames with any other starting byte trigger the diagnostic echo mechanism. |
| **Type** | 1 byte | Enum | **Service Type / Function Code**: <br>• `0x03`: Special & System Management functions (Ping / Handshake) <br>• `0x04`: Runtime Telemetry, Status & Control registers (`CodesRegistre`) <br>• `0x05`: Factory Calibration & Production (documented in `greenup-tools`) |
| **Opcode** | 1 byte | Enum | **Register / Subcommand Identifier**: Specifies the target register or function. |
| **Payload** | $0..N$ bytes | Variable | Optional arguments (in requests / writes) or register values (in responses). Empty for simple read queries. |
| **EOP** | 1 byte | `0xFF` | **End of Packet**: Delimiter placed immediately before the CRC bytes. |
| **CRC-16** | 2 bytes | 16-bit | **Checksum**: 16-bit Modbus CRC computed over all preceding bytes (`Byte 0` to `Byte L-2`, including `0xFF`). Transmitted in **Big-Endian** (`CRC_HI` then `CRC_LO`). |

---

### 3.2 Checksum Algorithm (CRC-16 Modbus / IBM)

- **Polynomial:** `0xA001` (reversed `0x8005`, $x^{16} + x^{15} + x^2 + 1$)
- **Initial Value:** `0xFFFF`
- **Bit Order:** Reflected (LSB first)
- **Serialization:** Big-Endian byte order (`CRC_HI`, `CRC_LO`)

#### Reference Python Implementation
```python
def calc_ble_crc16(data: bytes) -> int:
    """Computes standard CRC-16-IBM/Modbus across the provided byte sequence."""
    crc = 0xFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ 0xA001
            else:
                crc >>= 1
    return crc


def build_ble_frame(service_type: int, opcode: int, payload: bytes = b"") -> bytes:
    """Builds a fully-formed Legrand Green'up Bluetooth frame."""
    body = bytes([0x01, service_type, opcode]) + payload + bytes([0xFF])
    crc = calc_ble_crc16(body)
    return body + crc.to_bytes(2, "big")
```

---

### 3.3 Diagnostic Echo Mechanism

If the station receives incoming bytes that **do not start with `0x01`** (such as modem AT feedback or unsolicited strings like `+UUBTLC` emitted by u-blox NINA upon connection), it automatically echoes them back byte-for-byte.

Robust BLE clients should:
1. Drain/flush the GATT receive queue for 200–300 ms after establishing the BLE connection before sending the first packet.
2. Ignore any received notification whose payload is strictly identical to the transmitted request.

---

## 4. Special & Handshake Functions (`Type 0x03`)

Service Type `0x03` handles link keepalive and protocol handshakes:

| Opcode | Name | Request Payload | Response Payload | Description |
| :---: | :--- | :---: | :---: | :--- |
| `0x01` | `Ping` | *(None)* | `0xFF` (ACK) | **Keepalive Probe / Handshake**: Confirms link responsiveness and active session. Returns `0xFF` on success. |

> [!NOTE]
> Additional subcommands under Service `0x03` handling bootloader arming and OTA flashing signatures are intentionally omitted here to prevent accidental reboot into the bootloader, and are documented in the `greenup-tools` reference.

---

## 5. Runtime Telemetry & Control Registers (`Type 0x04`)

When `Type == 0x04`, the client communicates with the station's primary register map (`CodesRegistre`):
- **Read Query:** Transmit frame with empty payload: `[0x01, 0x04, Opcode, 0xFF, CRC_HI, CRC_LO]`. The station replies with the register value in the payload.
- **Write / Setpoint:** Transmit frame with argument payload: `[0x01, 0x04, Opcode, Value..., 0xFF, CRC_HI, CRC_LO]`.

### 5.1 Station Identity & Hardware Metadata

| Opcode | Register Name | Data Type | Description & Format |
| :---: | :--- | :---: | :--- |
| `0x01` | `NumeroSerie` | `uint32` (BE) | **Serial Number**: Big-endian integer (e.g. `0x003039` = 12345 representing `"012345"`). |
| `0x02` | `ReferenceBorne` | `uint16` (BE) | **Catalogue Reference**: 16-bit integer corresponding to the Legrand model (e.g. `58001` = 7.4 kW Single-phase with 6mA DC protection). |
| `0x03` | `NumeroTracabilite` | `byte[8]` | **Manufacturing Traceability**: Factory production and batch identifiers. |
| `0x04` | `VersionFirmware` | `byte[3]` | **Power Board Firmware**: 3 bytes `[Major, Minor, Build]` (e.g. `[1, 19, 2]` represents `V01.19.02`). |
| `0x05` | `VersionHardware` | `byte[3]` | **Hardware Revision**: 3 bytes representing the electronic board revision (e.g. `[2, 1, 1]` represents `V02.01.01`). |
| `0x06` | `PresenceKitCom` | `uint8` | **KitCom Presence Flag**: Reserved legacy stub (returns `0x00` in firmware). |
| `0x09` | `NomBorne` | `string` | **User Friendly Name**: ASCII/UTF-8 string (up to 10 characters, e.g. `"Legrand"`). |
| `0x0A` | `Horloge` | `byte[7]` | **Internal RTC**: `[Year, Month, Day, DayOfWeek, Hour, Minute, Second]`. |
| `0x0F` | `ChargingPointSide`| `uint8` | **Socket Side**: `0` = Left connector, `1` = Right connector (on dual-socket wallboxes). |
| `0x30` | `MacAdresse` | `byte[6]` | **Bluetooth MAC Address**: 6-byte hardware address of the u-blox transceiver. |
| `0x32` | `HardwareId` | `byte[]` | **Hardware GUID / Chip ID**: Unique factory silicon hardware ID. |

---

### 5.2 Operating States & Fault Diagnostics

| Opcode | Register Name | Data Type | Description & Values |
| :---: | :--- | :---: | :--- |
| `0x11` | `EtatCharge` | `uint8` | **High-Level Charging Status**: <br>• `0`: `NonConnecte` (Standby, no vehicle connected) <br>• `1`: `BrancheEnAttente_StartStop` (Cable plugged, waiting for start trigger) <br>• `2`: `BrancheEnAttente_Externe` (Waiting for supervisor authorization or schedule window) <br>• `3`: `BrancheEnAttente_VE` (Ready, waiting for vehicle demand) <br>• `4`: `EnCharge` (Contactor closed, active power delivered) <br>• `5`: `ChargeTerminee` (Vehicle battery full, current idle) <br>• `6`: `Off` (Station locked / disabled) <br>• `16`: `ErreurResetable` (Temporary mains voltage or frequency error) <br>• `17`: `ErreurNonResetable` (Hardware fault / contactor interlock error) |
| `0x12` | `ErreurCodification`| `uint8` | **Hardware Diagnostic Code**: <br>• `0`: No error <br>• `1`: Socket locking fault <br>• `2`: Socket unlocking fault <br>• `3`: Control Pilot short-circuit <br>• `4`: Control Pilot short on station side <br>• `5`/`6`: Domestic contactor fault <br>• `7`/`8`: EV contactor fault <br>• `9`: Missing vehicle diode <br>• `10`: Type 2 overcurrent <br>• `11`: Domestic Schuko overcurrent <br>• `12`: Mains power outage |
| `0x14` | `InfoDernierCharge` | `struct` | **Last Session Summary**: Peak-hours duration & energy (Wh), off-peak duration & energy (Wh), total energy delivered, and avoided CO2. |
| `0x15` | `EtatBorne` | `uint8` | **Operational State**: `0` = Inactive / Standby, `1` = Active / Ready. |

---

### 5.3 Charging Control, Setpoints & Scheduling

| Opcode | Register Name | Data Type | Description & Values |
| :---: | :--- | :---: | :--- |
| `0x16` | `CourantCharge` | `byte[]` / `uint8` | **Charging Current Limitation**: Supported current ratings array when read; sets target charging limit when written (e.g. `0x10` for 16A, `0x20` for 32A). |
| `0x17` | `ModeCharge` | `uint8` | **Charging Mode Configuration**: <br>• `0`: `Permanent` (Charging starts immediately upon connection) <br>• `2`: `Planned` (Charging restricted to active calendar windows) <br>• `16`: `Auto_HP_HC` (Synchronized with external dry-contact off-peak signal) <br>• `17`: `Auto_Linky` (Dynamically modulated via Enedis Linky TIC tele-information) |
| `0x18` | `ValeurPlanning` | `struct` | **Weekly Schedule Profiles**: Programmed charging windows (days of week, start hour/minute, end hour/minute, current rating). |
| `0x19` | `Charge` | `bool` | **Remote Start / Stop Command**: `1` = Authorize/start charge, `0` = Stop charge. |
| `0x20` | `DerogationCharge` | `bool` | **Temporary Charging Override**: `1` = Temporarily bypass schedule for immediate charge, `0` = Follow normal schedule. |
| `0x27` | `CourantSessionDeCharge`| `uint16` | **Active Session Current**: Real-time current delivered to the EV during active charge (in tenths of an Ampere). |
| `0x29` | `DecalageModePermanent`| `struct` | **Permanent Mode Offset**: Time offset parameters for scheduled permanent charging. |

---

## 6. Verified Wire Packet Traces

The following transactions illustrate validated requests and responses with exact CRC-16 checksums:

### 1. Keepalive Ping (`Type 0x03`, `Opcode 0x01`)
- **Request (Client $\rightarrow$ Station):**
  ```hex
  01 03 01 FF 08 B0
  ```
- **Response (Station $\rightarrow$ Client):**
  ```hex
  01 03 01 FF FF F4 49
  ```
  - Returns `0xFF` ACK payload with valid CRC.

---

### 2. Read Serial Number (`Type 0x04`, `Opcode 0x01`)
- **Request:**
  ```hex
  01 04 01 FF C9 01
  ```
- **Response (Serial `012345` / `0x003039`):**
  ```hex
  01 04 01 00 30 39 FF 65 9B
  ```
  - Payload: `0x003039` (Big-Endian) = `12345`

---

### 3. Read Catalogue Reference (`Type 0x04`, `Opcode 0x02`)
- **Request:**
  ```hex
  01 04 02 FF 39 01
  ```
- **Response (Reference `058001` / `0xE291`):**
  ```hex
  01 04 02 E2 91 FF 54 7C
  ```
  - Payload: `0xE291` (Big-Endian) = `58001` (Green'up Premium 7.4 kW single-phase)

---

### 4. Read Firmware Version (`Type 0x04`, `Opcode 0x04`)
- **Request:**
  ```hex
  01 04 04 FF 99 02
  ```
- **Response (`V01.17.78`):**
  ```hex
  01 04 04 01 11 4E FF 9D DF
  ```
  - Payload: `[0x01, 0x11, 0x4E]` = `[1, 17, 78]` $\rightarrow$ `V01.17.78`

---

### 5. Read Station Friendly Name (`Type 0x04`, `Opcode 0x09`)
- **Request:**
  ```hex
  01 04 09 FF 09 06
  ```
- **Response (`"Legrand"`):**
  ```hex
  01 04 09 4C 65 67 72 61 6E 64 00 00 00 FF 68 81
  ```
  - Payload: ASCII `"Legrand\0\0\0"`

---

### 6. Read Charging Status (`Type 0x04`, `Opcode 0x11`)
- **Request:**
  ```hex
  01 04 11 FF 09 0C
  ```
- **Response (Unplugged / Standby):**
  ```hex
  01 04 11 00 FF B5 08
  ```
  - Payload: `0x00` = `NonConnecte` (Unplugged)

---

### 7. Read Operating State (`Type 0x04`, `Opcode 0x15`)
- **Request:**
  ```hex
  01 04 15 FF C9 0E
  ```
- **Response (Active / Ready):**
  ```hex
  01 04 15 01 FF E4 48
  ```
  - Payload: `0x01` = `Actif`

---

### 8. Read Allowed Current Steps (`Type 0x04`, `Opcode 0x16`)
- **Request:**
  ```hex
  01 04 16 FF 39 0E
  ```
- **Response (`[16A, 20A, 25A, 32A, 32A]`):**
  ```hex
  01 04 16 10 14 19 20 20 FF D7 1C
  ```
  - Payload: Array of current ratings in Amperes.

---

## 7. Summary Comparison: Serial (`USART1`) vs Bluetooth (`USART0`)

| Feature | Physical Serial (`USART1`) | Bluetooth Low Energy (`USART0`) |
| :--- | :--- | :--- |
| **Physical Interface** | Internal USB cable (`USART1`) | u-blox NINA-B112 BLE module (`USART0`) |
| **Baud Rate** | 115200 bauds (8N1) | 115200 bauds (8N1) |
| **Framing** | Human-readable ASCII (`\r\n`) | Structured Binary (`0x01` ... `0xFF` + CRC-16) |
| **Integrity Check** | None (Direct physical wire) | Mandatory CRC-16-IBM / Modbus (`0xA001`) |
| **Paradigm** | **Asynchronous Push & Pull** | **Strict Polling (Pull Only)** |
| **Spontaneous Events**| Yes (`State:X`, `CCI:XX`, tickets) | **None** (Silent unless queried) |
| **Range / Medium** | Wired (Local Raspberry Pi / host) | Wireless 2.4 GHz (Smartphone proximity) |
| **Green'up Link Use** | **Primary active supervision port** | Documented for reference / external tools |
