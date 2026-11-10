# Legrand Bluetooth Low Energy (BLE) Protocol — Complete Reference

> [!NOTE]
> **Green'up Link Architecture & Scope:**  
> Green'up Link connects directly to the charging station's physical UART interface (`USART1`, via an internal USB cable) rather than using Bluetooth Low Energy. The physical serial bus is faster, supports unsolicited asynchronous push events (real-time telemetry and state changes), and provides full bidirectional supervision without wireless range or pairing constraints.  
> This document describes the binary BLE protocol for informational purposes, protocol completeness, and to serve as a comprehensive reference for third-party or community projects targeting the wireless interface.

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
| **Type** | 1 byte | Enum | **Service Type / Function Code**: <br>• `0x03`: Special & System Management functions (Ping, Reset, Decommissioning) <br>• `0x04`: Read Telemetry & Status registers <br>• `0x05`: Write / Configure station parameters |
| **Opcode** | 1 byte | Enum | **Command Identifier**: Specifies the exact register or function to execute. |
| **Payload** | $0..N$ bytes | Variable | Optional arguments (in requests) or register values (in responses). May be empty. |
| **EOP** | 1 byte | `0xFF` | **End of Packet**: Delimiter placed immediately before the CRC bytes. |
| **CRC-16** | 2 bytes | 16-bit | **Checksum**: 16-bit Modbus CRC computed over all preceding bytes (`Byte 0` to `Byte L-2`, including `0xFF`). Transmitted in **Big-Endian** (`CRC_HI` then `CRC_LO`). |

---

### 3.2 Checksum Algorithm (CRC-16 Modbus / IBM)

- **Polynomial:** `0xA001` (reversed `0x8005`, $x^{16} + x^{15} + x^2 + 1$)
- **Initial Value:** `0xFFFF`
- **Bit Order:** Reflected (LSB first)
- **Serialization:** Big-Endian byte order

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

If the station receives incoming bytes that **do not start with `0x01`** (such as modem AT feedback or arbitrary ASCII strings like `PING\r\n`), it automatically echoes them back byte-for-byte.

Robust BLE clients should:
1. Drain/flush the GATT receive queue for 200–300 ms after establishing the BLE connection before sending the first packet.
2. Ignore any received notification whose payload is strictly identical to the transmitted request.

---

## 4. Special & System Functions (`Type 0x03`)

Special functions handle connection keepalive, software reboot, and smartphone pairing lifecycle:

| Opcode | Name | Request Payload | Response Payload | Description |
| :---: | :--- | :---: | :---: | :--- |
| `0x01` | `Ping` | *(None)* | `0xFF` (ACK) | **Keepalive Probe**: Confirms link responsiveness. Returns `0xFF` on success. |
| `0x02` | `Reset` | *(None)* | `0xFF` (ACK) | **Software Reboot**: Reboots the station microcontroller and re-runs boot autotests. |
| `0x03` | `Decommissioning` | *(None)* | `0xFF` (ACK) | **Unpair / Factory Reset**: Clears stored smartphone pairing keys and resets user settings. |

---

## 5. Telemetry & Read Registers (`Type 0x04`)

When `Type == 0x04`, the client queries station metadata, electrical measurements, and operational states:

### 5.1 Station Identity & Hardware Metadata

| Opcode | Name | Data Type | Description & Format |
| :---: | :--- | :---: | :--- |
| `0x01` | `SerialNumber` | `uint32` (BE) | **Serial Number**: 4-byte big-endian integer (e.g. `0x003039` = 12345 representing `"012345"`). |
| `0x02` | `Reference` | `uint16` (BE) | **Catalogue Reference**: 16-bit integer corresponding to the Legrand model (e.g. `58001` = 7.4 kW Single-phase with 6mA DC protection). |
| `0x03` | `Traceability` | `byte[8]` | **Manufacturing Batch**: Factory production and traceability identifiers. |
| `0x04` | `FirmwareVersion` | `byte[3]` | **Power Board Firmware**: 3 bytes `[Major, Minor, Build]` (e.g. `[1, 19, 2]` represents `V01.19.02`). |
| `0x05` | `HardwareVersion` | `byte[3]` | **Hardware Revision**: 3 bytes representing the electronic board revision (e.g. `[2, 1, 1]` represents `V02.01.01`). |
| `0x06` | `KitComPresence` | `uint8` | **KitCom Presence Flag**: Reserved legacy stub (returns `0x00`). |
| `0x09` | `StationName` | `string` | **User Friendly Name**: ASCII/UTF-8 string (up to 10 characters, e.g. `"Legrand"`). |
| `0x0A` | `RealTimeClock` | `byte[7]` | **Internal RTC**: `[Year, Month, Day, DayOfWeek, Hour, Minute, Second]`. |
| `0x0F` | `ConnectorSide` | `uint8` | **Socket Side**: `0` = Left connector, `1` = Right connector (on dual-socket wallboxes). |
| `0x30` | `MacAddress` | `byte[6]` | **Bluetooth MAC Address**: 6-byte hardware address of the u-blox transceiver. |

---

### 5.2 Operating States & Fault Diagnostics

| Opcode | Name | Data Type | Description & Values |
| :---: | :--- | :---: | :--- |
| `0x11` | `ChargeStatus` | `uint8` | **High-Level Charging Status**: <br>• `0`: `Unplugged` (Standby, no vehicle connected) <br>• `1`: `WaitingStart` (Cable plugged, waiting for start trigger) <br>• `2`: `WaitingExternal` (Waiting for supervisor authorization or schedule window) <br>• `3`: `WaitingVehicle` (Ready, waiting for vehicle demand) <br>• `4`: `Charging` (Contactor closed, active power delivered) <br>• `5`: `ChargeComplete` (Vehicle battery full, current idle) <br>• `6`: `Disabled` (Station locked / disabled) <br>• `16`: `ResettableFault` (Temporary mains voltage or frequency error) <br>• `17`: `PermanentFault` (Hardware fault / contactor interlock error) |
| `0x12` | `ErrorCode` | `uint8` | **Hardware Diagnostic Code**: <br>• `0`: No error <br>• `1`: Socket locking fault <br>• `2`: Socket unlocking fault <br>• `3`: Control Pilot short-circuit <br>• `7` / `8`: Power contactor actuation fault <br>• `9`: Missing vehicle diode <br>• `10`: Type 2 overcurrent <br>• `11`: Domestic Schuko overcurrent <br>• `12`: Mains power outage |
| `0x14` | `LastSessionReport` | `struct` | **Last Session Summary**: Session duration, peak-hours energy (Wh), off-peak energy (Wh), and total energy delivered. |
| `0x15` | `StationState` | `uint8` | **Operational State**: `0` = Standby / Inactive, `1` = Active / Ready. |

---

### 5.3 Current Limits, Modes & Control

| Opcode | Name | Data Type | Description & Values |
| :---: | :--- | :---: | :--- |
| `0x16` | `AvailableCurrents` | `byte[]` | **Supported Current Ratings**: List of available current ratings supported by the station configuration (e.g. `[16, 20, 25, 32]`). |
| `0x17` | `ChargeMode` | `uint8` | **Configured Charging Mode**: <br>• `0`: `Immediate` (Charging starts immediately upon connection) <br>• `2`: `Scheduled` (Charging restricted to active calendar windows) <br>• `16`: `AutoOffPeak` (Synchronized with external dry-contact off-peak signal) <br>• `17`: `AutoLinky` (Dynamically modulated via Enedis Linky TIC tele-information) |
| `0x18` | `WeeklySchedule` | `struct` | **Weekly Schedule Profiles**: Programmed charging windows (days of week, start hour/minute, end hour/minute, current rating). |
| `0x19` | `ChargeAuthorization` | `bool` | **Charge Authorization State**: `0` = Inactive / Disabled, `1` = Authorized. |
| `0x20` | `Derogation` | `bool` | **Immediate Override**: `1` if a temporary override bypasses schedule boundaries for the current session; `0` otherwise. |
| `0x27` | `ActiveSessionCurrent` | `uint16` (BE) | **Real-Time Current**: Measured current flowing into the vehicle during active charging (in tenths of an Ampere). |

---

## 6. Write & Configuration Commands (`Type 0x05`)

When `Type == 0x05`, the client writes configuration setpoints and control commands to the station:

| Opcode | Target Function | Payload | Description |
| :---: | :--- | :---: | :--- |
| `0x16` | `SetCurrentLimit` | `uint8` (Amperes) | Sets the charging current limit (e.g. `0x10` for 16A, `0x20` for 32A). |
| `0x17` | `SetChargeMode` | `uint8` (Mode Code) | Configures charging mode (`0` = Immediate, `2` = Scheduled, `16` = Off-Peak, `17` = Linky TIC). |
| `0x18` | `SetWeeklySchedule` | `struct` | Writes weekly timer profiles for scheduled charging. |
| `0x19` | `SetChargeAuthorization`| `uint8` (`0` or `1`) | Remotely authorizes (`1`) or inhibits (`0`) charging. |
| `0x20` | `SetDerogation` | `uint8` (`0` or `1`) | Activates (`1`) or clears (`0`) temporary schedule override. |

---

## 7. Example Packet Exchanges

All examples below show exact wire transactions with validated CRC-16 checksums using generic test vectors:

### 1. Keepalive Ping (`Type 0x03`, `Opcode 0x01`)
- **Request (Client $\rightarrow$ Station):**
  ```hex
  01 03 01 FF 08 B0
  ```
  - `0x01`: SOH
  - `0x03`: Special Function
  - `0x01`: Ping Sub-command
  - `0xFF`: EOP
  - `0x08 0xB0`: CRC-16 Modbus (Big-Endian)
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
- **Response (Example Serial `012345` / `0x003039`):**
  ```hex
  01 04 01 00 30 39 FF 6B 8A
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
- **Response (`V01.19.02`):**
  ```hex
  01 04 04 01 13 02 FF 29 8B
  ```
  - Payload: `[0x01, 0x13, 0x02]` = `[1, 19, 2]` $\rightarrow$ `V01.19.02`

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
  - Payload: `0x00` = `Unplugged`

---

### 7. Read Allowed Current Steps (`Type 0x04`, `Opcode 0x16`)
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

## 8. Summary Comparison: Serial (`USART1`) vs Bluetooth (`USART0`)

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
