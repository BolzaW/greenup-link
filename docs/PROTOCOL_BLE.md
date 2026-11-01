# Legrand Bluetooth Low Energy (BLE) Protocol (Green'up Power Board) — Complete Reference

> **Firmware Reference:** Microchip / Atmel **ATmega2560** running official Legrand firmware (`V01.19.02`, `V01.18.04`, `V01.17.77`).  
> **Physical Interface:** `USART0` (`Serial` in firmware, Pins `PE0`/RXD0 [pin 2] and `PE1`/TXD0 [pin 3]) at **115200 bauds, 8 data bits, no parity, 1 stop bit (8N1)**.  
> **Hardware Transceiver:** **u-blox NINA-B112** (Bluetooth Low Energy 4.2 / 5.0) operating in Serial Port Service (SPS) / Data Mode.  
> **Communication Paradigm:** Strictly **Half-Duplex Master-Slave Request-Response (Polling / Pull)**.  
>
> *Note on Dual-Interface Separation:* The ATmega2560 features two completely separated serial communications:
> 1. `USART1` (`Serial1` on `PD2`/`PD3`): The **ASCII Text Supervisory Protocol** used by the KitCom / Raspberry Pi (see [`LEGRAND_SERIAL_PROTOCOL.md`](./LEGRAND_SERIAL_PROTOCOL.md)).
> 2. `USART0` (`Serial` on `PE0`/`PE1`): The **Binary Framed RPC Protocol** documented here, dedicated exclusively to the u-blox BLE module and the Legrand "EV Charge" smartphone application.

> [!NOTE]
> ### ✅ Hardware Validated Milestone (Live Physical Testing)
> The core binary framing, CRC-16 Modbus checksum algorithm, and serial turnaround mechanics documented below have been **physically validated on live charging station hardware** (Legrand Green'up Premium at `54:F8:2A:57:DD:71` running ATmega2560 firmware `V01.19.02` with u-blox NINA-B112 transceiver running `u-connectXpress 5.0.0-011`).
> - **Live Handshake Confirmation:** The keepalive probe (`CodesFonction.Special` `0x03`, `Ping` `0x01`) responded in real-time with `01 03 01 FF FF F4 49` (ACK payload `0xFF`, CRC verified mathematically).
> - **Register Map Synchronization:** All command opcodes and payload structures have been cross-verified against official Android / Xamarin assemblies (`Exakis.Legrand.GreenUp.dll`) and correlated with the ATmega2560 internal jump table (`0x146A6`).

---

## 🚦 Architectural Overview & Fundamental Behavior

Unlike the Raspberry Pi supervisory port (`Serial1`) which pushes unsolicited asynchronous text events (`State:B\r`, `CCI:016\r`, `FCS:1\r`, session tickets), the Bluetooth interface on `USART0` is **completely passive**:

1. **No Spontaneous Frames (Strict Polling):**
   The main loop dispatcher (`manage_bluetooth()` at `0x14528` in Flash) only executes when incoming bytes are available (`Serial.available() > 0`). If no request is received from the smartphone, the ATmega **never sends unsolicited data**.
2. **Noise Immunity & Integrity Check (Mandatory CRC-16):**
   Because the transceiver operates inside an electrical enclosure subject to 32 A inductive switching noise, every single packet (both inbound and outbound) is strictly validated by an industrial **CRC-16-IBM / Modbus** checksum. Any frame with an invalid checksum or malformed header is instantly purged from SRAM without side-effects.
3. **Internal RAM Convergence:**
   Although the wire protocols are totally incompatible (ASCII text vs. binary frames), the Bluetooth handlers read and mutate the **exact same internal state variables** in SRAM as the supervisor commands:
   - `0x0300`: `g_current_state` (finite-state machine 'A'..'Z')
   - `0x0DAF`: `g_autorisation_charge` (charge authorization flag, set by `DOK` or BLE Cmd `0x27`)
   - `0x0D47`: `g_socket_lock_state` (T2S socket locking solenoid)
   - `0x0C23`–`0x0C32`: Active energy counters (kWh / TIC Linky totalizers)

---

## 🛰️ Modem Setup: u-blox NINA AT Sequence & Hardware Control

During board initialization and upon receiving `BTOK` on `USART1` (`0x19EB6`–`0x1A2CE` in Flash), the ATmega controls the power and reset lines before configuring the u-blox NINA BLE transceiver via standard Hayes `AT+` commands:

### 1. Hardware Pin Control
* **Pin 34 (D34 / `0x1F15A`):** NINA Power Enable. Driven HIGH to power the module, driven LOW by `BTNOK` (`0x14518`–`0x14524`) to completely shut down the transceiver.
* **Pin 35 (D35 / `0x19EEA`–`0x19F02`):** NINA Hardware Reset. Pulled LOW (100 ms) then HIGH (100 ms) to trigger a clean hardware reboot.

### 2. AT Configuration Sequence (in exact execution order)
| Step | Command Sent to u-blox | Purpose & Exact Parameters |
| :---: | :--- | :--- |
| 1 | `AT+UBTLN="Green'up-XXXXXX"\r` | Sets the broadcast BLE Local Name, derived dynamically from the board's serial number stored in EEPROM (`0x02E7`..`0x02ED`). |
| 2 | `AT+UMRS=115200,2,8,1,1,1\r` | Sets UART parameters: `115200` bauds, flow control `2` (CTS/RTS disabled), `8` data bits, `1` stop bit, parity `1` (None), `1` (apply immediately). |
| 3 | `AT+UMSM=1\r` | Sets default startup mode to **Data Mode (Serial Port Service / SPS)**. |
| 4 | `AT&W0\r` | Saves the entire configuration into the NINA non-volatile flash profile 0. |
| 5 | `AT+UMLA=1\r` | Reads the factory Bluetooth MAC address of the NINA chip. |
| 6 | `AT+CPWROFF\r` | Soft-reboots the NINA transceiver so that Profile 0 takes effect, booting straight into transparent SPS Data Mode. |

Once the NINA transceiver reboots in Data Mode, all bytes exchanged over `USART0` bypass the Hayes AT parser and are streamed directly across the BLE GATT Serial Port Service.

---

## 📶 BLE GATT Architecture (u-blox SPS Specification UBX-16011192)

The u-blox NINA transceiver exposes the following standardized GATT services and characteristics:

### 1. u-blox Serial Port Service (SPS) — Service `2456e1b9-26e2-8f83-e744-f34f01e9d701`
* **FIFO Data Characteristic (`2456e1b9-26e2-8f83-e744-f34f01e9d703`):**
  - **Properties:** `READ`, `WRITE WITHOUT RESPONSE`, `WRITE`, `NOTIFY`.
  - **Downstream (Central $\rightarrow$ Station):** Written via **GATT Write Commands (`response=False`)**. Writes are emitted directly onto the ATmega `USART0` RX pin (`PE0`).
  - **Upstream (Station $\rightarrow$ Central):** Packets output by the ATmega on `USART0` TX pin (`PE1`) are pushed to the Central as **GATT Notifications**. The Central MUST subscribe to notifications on this characteristic before transmitting.
* **Credits Characteristic (`2456e1b9-26e2-8f83-e744-f34f01e9d704`):**
  - **Properties:** `WRITE WITHOUT RESPONSE`, `WRITE`, `NOTIFY`.
  - **Transparent Mode (Default / Recommended):** If the client never writes to this characteristic, the module operates in **Connection Without Credits**. Data is relayed with zero handshake overhead.
  - **Credit-Based Flow Control Mode:** If used, the Central must strictly follow section 4.3.1 of `UBX-16011192`: enable notifications on both FIFO and Credits *before* writing initial credits, then wait for the peripheral's credit notification. Writing credits without enabling notifications causes packet lockups.

### 2. Device Information Service — Service `0000180a-0000-1000-8000-00805f9b34fb`
Standard readable descriptors giving transceiver identity:
- `00002a24-0000-1000-8000-00805f9b34fb`: Model Number (e.g. `NINA-B112`)
- `00002a26-0000-1000-8000-00805f9b34fb`: Firmware Revision (e.g. `u-connectXpress v...`)
- `00002a28-0000-1000-8000-00805f9b34fb`: Software Revision
- `00002a29-0000-1000-8000-00805f9b34fb`: Manufacturer Name (`u-blox`)

---

## 🔁 Raw USART0 Echo Diagnostic (`0x16AC0`–`0x16B36`)

As a diagnostic mechanism embedded in the firmware dispatcher, if the ATmega receives incoming bytes that **do not start with the `0x01` SOH delimiter**, it collects them into an SRAM buffer (up to 49 octets or 101 ms timeout) and **echoes them back byte-for-byte** via `Serial.write()`:

```
Central (BLE Write)  -->  NINA-B112  -->  ATmega2560 RX0 (PE0)
                                                |
                                    [Byte 0 != 0x01 ?]
                                                |
                                        (Echo Loop 0x16B1E)
                                                |
Central (BLE Notify) <--  NINA-B112  <--  ATmega2560 TX0 (PE1)
```

Sending an arbitrary ASCII string like `PING\r\n` provides immediate physical validation that the internal `USART0` serial bus is alive and communicating at 115200 bauds.

---

## 📦 Packet Framing & CRC-16 Specification

Every binary frame (request and response) follows a strict multi-byte envelope:

```
+--------+--------+----------------+--------------------------+--------+----------+----------+
| Byte 0 | Byte 1 |     Byte 2     | Bytes 3 .. (Len - 4)     | Byte L-2| Byte L-1 |  Byte L  |
|  SOH   |  Type  | Opcode/Subcmd  | Payload Data (0..N bytes)|  EOP   |  CRC_HI  |  CRC_LO  |
| (0x01) | (1 o)  |     (1 o)      |         (Variable)       | (0xFF) |  (1 o)   |  (1 o)   |
+--------+--------+----------------+--------------------------+--------+----------+----------+
```

### 1. Frame Envelope Details

* **Byte 0 — SOH (`0x01` / `DebutTrame`):** Start of Header. Any packet starting with an octet other than `0x01` is rejected immediately (`0x16AC0`), and input bytes are echoed back on `USART0` as raw diagnostic.
* **Byte 1 — Service / Packet Type (`CodeFonction`):**
  - `0x03` : **Special Functions (`CodesFonction.Special`)**:
    - Sub-Opcode `0x01` (`CodeSpecialRegistre.Ping`): Harmless keepalive handshake.
    - Sub-Opcode `0x02` (`CodeSpecialRegistre.Reset`): Software reset of the charging station.
    - Sub-Opcode `0x03` (`CodeSpecialRegistre.Decommissioning`): Factory reset / unpair smartphone.
  - `0x04` : **Runtime Application Telemetry & Read (`CodesFonction.Read`)**: Standard register querying (EV Charge app).
  - `0x05` : **Register Configuration & Write (`CodesFonction.Write`)**: Writing setpoints, schedules, and mode parameters.
* **Byte 2 — Opcode / Command Identifier (`CodesRegistre` or `CodeSpecialRegistre`):**
  Identifies the specific RPC method (e.g., `0x01` for Ping when Type=0x03; `0x01` for Serial Number, `0x11` for Charge Status, `0x15` for Operational State, `0x23` for LancerMiseAJour when Type=0x04).
* **Bytes 3 .. $L-3$ — Payload Data (`Data`):**
  Optional command arguments (in request) or telemetry register values (in response). Can be empty (0 bytes).
* **Byte $L-2$ — EOP (`0xFF` / `FinTrame`):**
  End of Packet delimiter. Placed **before** the CRC bytes.
* **Bytes $L-1$ & $L$ — CRC-16 Checksum (Big-Endian):**
  16-bit Modbus CRC calculated over all preceding bytes (`Byte 0` up to `Byte L-2`, including `0xFF`).
  Formatted in **Big-Endian** (Most Significant Byte `CRC_HI` first, then Least Significant Byte `CRC_LO`).

> [!IMPORTANT]
> ### ⚙️ Hardware Reception Mechanism (ATmega2560 `manage_bluetooth` at `0x14600`–`0x14676`)
> Disassembly of the ATmega firmware reveals the exact frame boundary detection logic:
> 1. In incoming bytes streaming through `Serial.read()`, byte $N$ is appended to the SRAM buffer at `Y + 0x33 + N`.
> 2. The firmware inspects `buffer[N - 2]`:
>    ```avr
>    0x14628: ldi r30, 0x32
>    0x1462C: add r30, r28 ; adc r31, r29  ; Z = Y + 0x32
>    0x14630: add r30, r4  ; adc r31, r5   ; Z += r4 (N - 1) -> buffer[N - 2]
>    0x14634: ld  r24, Z
>    0x14638: cpi r24, 0xFF                ; Was the byte 2 steps ago 0xFF?
>    0x1463A: breq 0x14646                 ; If YES -> Packet complete, trigger CRC check!
>    ```
> 3. Once `buffer[N - 2] == 0xFF`, the microcontroller halts reception, computes CRC on the preceding $N-1$ octets (all bytes including `0xFF`) via `0x030FA`, performs a 2-byte XOR swap on the received CRC bytes (`0x1466A`–`0x1466E`), and verifies match.
> 4. If a frame mistakenly places `0xFF` at the very end of the packet, `buffer[N - 2]` will never evaluate to `0xFF` upon completion, leading to an **infinite wait / timeout**.

---

### 2. Checksum Algorithm (CRC-16 Modbus / IBM)

The integrity function at [`0x030FA`](file:///c:/Users/rapha/Desktop/Maj%20borne%20legrand/retro_ingenierie/legrand/hex/FirmwareBoardA-V01;19;02.hex#L784) implements standard CRC-16-IBM / Modbus:

* **Polynomial:** `0xA001` (reversed `0x8005`, $x^{16} + x^{15} + x^2 + 1$)
* **Initial Value:** `0xFFFF`
* **Input / Output:** Reflected (LSB first)
* **Serialization:** Big-Endian byte order (`crc.to_bytes(2, "big")` / C# `crc.ToString("X4")` -> `HexStringToByteArray`)

#### Reference Implementation (Python)
```python
def calc_ble_crc16(data: bytes) -> int:
    crc = 0xFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ 0xA001
            else:
                crc >>= 1
    return crc


def build_frame(service_type: int, opcode: int, payload: bytes = b"") -> bytes:
    """Builds a valid Legrand Green'up Bluetooth frame."""
    body = bytes([0x01, service_type, opcode]) + payload + bytes([0xFF])
    crc = calc_ble_crc16(body)
    return body + crc.to_bytes(2, "big")
```

---

## 📱 Service Type 0x04: Runtime Telemetry & Supervision (CodesRegistre)

When `Byte 1 == 0x04` (`CodesFonction.Read`), the central centralizes all supervisory requests to the ATmega2560.
The ATmega internal jump table (`__tablejump_elpm__` at `0x22590`, dispatching from table words at `0x146A6`) indexes commands from `0x01` to `0x45` (1 to 69).

The exact register IDs and payload types correspond directly to the official Legrand C# assembly specification (`Exakis.Legrand.GreenUp.Domain.BluetoothCommand.Base.CodesRegistre`):

### 1. Station Identification & Hardware Metadata

| Opcode | Register Name (`CodesRegistre`) | Jump Target | Data Type | Description & Payload Format |
| :---: | :--- | :---: | :---: | :--- |
| `0x01` (1) | `NumeroSerie` | `0x148F8` | `long` / ASCII | **Serial Number**: 4-byte / 8-byte integer or ASCII serial number string of the station. |
| `0x02` (2) | `ReferenceBorne` | `0x14A2C` | `uint16` / Enum | **Commercial Reference Code**: 16-bit integer mapped to `ReferenceBorne` (e.g., `59001` = 7.4 kW Mono standard, `59002` = 22 kW Tri, `58001` = 7.4 kW 6mA DC). |
| `0x03` (3) | `NumeroTracabilite` | `0x14C02` | `struct` | **Manufacturing Traceability**: Factory manufacturing batch and traceability identifiers (`FirstNumber`, `SecondNumber`). |
| `0x04` (4) | `VersionFirmware` | `0x14C26` | `byte[3]` | **Firmware Version**: 3 bytes `[Major, Minor, Build]` (e.g. `[1, 19, 2]` represents `v1.19.02`). |
| `0x05` (5) | `VersionHardware` | `0x14CA0` | `byte[3]` | **Hardware Version**: 3 bytes representing the physical electronic board revision. |
| `0x06` (6) | `PresenceKitCom` | `0x14F8C` / `0x13944` | `bool` | **KitCom Presence Flag (Inexploitable / Stub)**: Toujours à `0x00` en dur (`std Y+4, r1`). Fonction abandonnée / non finalisée par Legrand côté BLE. Déconnectée de la présence réelle `g_kit_com_present` (`0x0DB0`) sur le port série `USART1`. |
| `0x07` (7) | `AutoModePossibilite` | `0x14F8C` / `0x13944` | `uint8` | **Automatic Capabilities (Stub)**: Même stub codé en dur à `0x00`. |
| `0x08` (8) | `GestionConsigne` | `0x14D14` | `uint8` | **Gestion Consigne**: Paramètres de modulation de puissance. |
| `0x09` (9) | `NomBorne` | `0x148AE` | `string` | **Station Friendly Name**: UTF-8 user-assigned string (e.g., `"Legrand"`). |
| `0x0A` (10) | `Horloge` | `0x14D48` | `byte[7]` | **Real-Time Clock (RTC)**: Internal clock timestamp (Year, Month, Day, DayOfWeek, Hours, Minutes, Seconds). |
| `0x0F` (15) | `ChargingPointSide` | `0x15806` | `uint8` | **Point Side**: `0` = Left connector, `1` = Right connector (for dual-socket stations). |
| `0x30` (48) | `MacAdresse` | `0x15760` | `byte[6]` | **Transceiver MAC Address**: Factory Bluetooth MAC address of the u-blox NINA module. |
| `0x32` (50) | `HardwareId` | `0x15806` | `byte[]` | **Hardware GUID / Chip ID**: Unique factory silicon hardware ID. |

---

### 2. Operational State, Charging Status & Diagnostics

| Opcode | Register Name (`CodesRegistre`) | Jump Target | Data Type | Description & Payload Format |
| :---: | :--- | :---: | :---: | :--- |
| `0x11` (17) | `EtatCharge` | `0x14E12` | `uint8` | **Charge Status (`ChargeStatus`)**: Current charging state: <br>• `0`: `NonConnecte` (unplugged) <br>• `1`: `BrancheEnAttente_StartStop` <br>• `2`: `BrancheEnAttente_Externe` (waiting supervisor `DOK`) <br>• `3`: `BrancheEnAttente_VE` (vehicle plugged, ready) <br>• `4`: `EnCharge` (contactor closed, active power delivery) <br>• `5`: `ChargeTerminee` (vehicle full, charging complete) <br>• `6`: `Off` (station disabled) <br>• `16`: `ErreurResetable` (voltage/frequency fault) <br>• `17`: `ErreurNonResetable` (emergency stop/welded contactor) <br>• `255`: `Rebooting` |
| `0x12` (18) | `ErreurCodification` | `0x15806` | `uint8` | **Hardware Fault Diagnosis (`ErreurCodification`)**: <br>• `0`: No error <br>• `1`: Socket locking fault <br>• `2`: Socket unlocking fault <br>• `3`: CP pilot wire short-circuit <br>• `4`: CP short on station side <br>• `5`/`6`: Domestic contactor opening/closing fault <br>• `7`/`8`: EV contactor opening/closing fault <br>• `9`: Missing vehicle diode <br>• `10`: EV plug overcurrent <br>• `11`: Domestic plug overcurrent <br>• `12`: Power outage / mains blackout <br>• `255`: Waiting firmware update |
| `0x14` (20) | `InfoDernierCharge` | `0x14F82` | `struct` | **Last Session Charge Report (`RapportCharge`)**: Detailed energy metrics: Peak hours (`HP_NombreHeure`, `HP_NombreMinutes`, `HP_Consommation`), Off-peak hours (`HC_NombreHeure`, `HC_NombreMinutes`, `HC_Consommation`), total energy in Wh, CO2 avoided. |
| `0x15` (21) | `EtatBorne` | `0x14F94` | `uint8` | **Station Operational Switch (`EtatBorne`)**: `0` = Inactive / Standby, `1` = Active / Ready. |

---

### 3. Charging Control, Setpoints & Scheduling

| Opcode | Register Name (`CodesRegistre`) | Jump Target | Data Type | Description & Payload Format |
| :---: | :--- | :---: | :---: | :--- |
| `0x16` (22) | `CourantCharge` | `0x14FB2` | `byte[]` | **Charging Current Limitation**: Setpoint current limit assigned to the vehicle (in Amperes, e.g. `10`, `14`, `16`, `20`, `25`, `32`). Supports both single setpoint read and list of authorized steps. |
| `0x17` (23) | `ModeCharge` | `0x14FE6` | `uint8` | **Charge Mode Configuration (`ModeCharge`)**: <br>• `0`: `Permanent` (charge starts immediately upon connection) <br>• `2`: `Planned` (charge restricted to active scheduling windows) <br>• `16`: `Auto_HP_HC` (synchronization with dry-contact off-peak input) <br>• `17`: `Auto_Linky` (dynamic modulation via TIC tele-information demodulator) |
| `0x18` (24) | `ValeurPlanning` | `0x150EC` | `struct` | **Charge Scheduling Configuration (`BornePlanning`)**: Active timer profiles: days of week, start hour/quarter-hour, end hour/quarter-hour, max current limit during window. |
| `0x19` (25) | `Charge` | `0x15806` | `bool` | **Remote Start / Stop Charge Command**: Start or stop charging remotely via app. |
| `0x20` (32) | `DerogationCharge` | `0x150FC` | `bool` | **Temporary Charging Override**: Temporarily bypasses off-peak or planning constraints for immediate single-session charging. |
| `0x23` (35) | `LancerMiseAJour` | `0x1532A` / `0x13CDC` | `byte[1]` | **Trigger Firmware Update (Internal)**: Commande interne réservée au processus officiel de mise à jour constructeur. Non supportée dans ce pilote opérationnel de supervision. |
| `0x24` (36) | `ReseauKitComUtilise` | `0x14F8C` | `uint8` | **Active Network Interface (`NetworkType`)**: `0` = None, `1` = RJ45 Ethernet, `2` = WiFi, `3` = Both, `4` = Configure Later. |
| `0x27` (39) | `CourantSessionDeCharge`| `0x1539C` | `uint16` | **Active Session Current**: Real-time current delivered to the EV during the ongoing charging cycle. |
| `0x29` (41) | `DecalageModePermanent`| `0x15806` | `struct` | **Permanent Mode Offset**: Time offset parameters for scheduled permanent charging. |

---

### 4. Verified Live Hardware Packet Traces

The following transactions have been captured and validated on physical hardware:

#### A. Keepalive Handshake (`Type 0x03`, `Opcode 0x01` — `CodesFonction.Special.Ping`)
* **Request (Central $\rightarrow$ Station):**
  ```hex
  01 03 01 FF 08 B0
  ```
  - `0x01` : SOH (`DebutTrame`)
  - `0x03` : Service `Special`
  - `0x01` : Sub-command `Ping`
  - `0xFF` : EOP delimiter
  - `0x08 0xB0` : CRC-16 Modbus (Big-Endian)
* **Response (Station $\rightarrow$ Central):**
  ```hex
  01 03 01 FF FF F4 49
  ```
  - Non-destructive immediate ACK (`Payload: FF`, CRC: `0xF449`). Handshake validated!

#### B. Numéro de Série (`Type 0x04`, `Opcode 0x01` — `NumeroSerie`)
* **Request:** `01 04 01 FF C9 01`
* **Response:** `01 04 01 00 3B 50 FF 09 3A`
  - Payload: `0x003B50` (Big-Endian) = `15184` (Serial Number `"015184"`)

#### C. Référence Commerciale (`Type 0x04`, `Opcode 0x02` — `ReferenceBorne`)
* **Request:** `01 04 02 FF 39 01`
* **Response:** `01 04 02 E2 91 FF 54 7C`
  - Payload: `0xE291` (Big-Endian) = `58001` (Green'up Premium 7.4 kW Monophasé 32A avec diff. 6mA DC)

#### D. Version Firmware Carte Mère (`Type 0x04`, `Opcode 0x04` — `VersionFirmware`)
* **Request:** `01 04 04 FF 99 02`
* **Response:** `01 04 04 01 11 4E FF 9D DF`
  - Payload: `[0x01, 0x11, 0x4E]` = `v1.17.78` (`0x11` = 17, `0x4E` = 78)

#### E. Révision Hardware (`Type 0x04`, `Opcode 0x05` — `VersionHardware`)
* **Request:** `01 04 05 FF 09 03`
* **Response:** `01 04 05 02 01 01 FF EC D7`
  - Payload: `[0x02, 0x01, 0x01]` = `v2.01.01`

#### F. Présence Carte KitCom (`Type 0x04`, `Opcode 0x06` — `PresenceKitCom`)
* **Request:** `01 04 06 FF F9 03`
* **Response:** `01 04 06 00 FF B1 B8`
  - Payload: `0x00`

> [!WARNING]
> **Valeur inexploitable — Registre codé en dur à `0x00` (Fonction non développée entièrement par Legrand) :**  
> L'analyse approfondie du désassemblage du firmware ATmega2560 (`0x13944` en `v1.17`, `0x14F8C` en `v1.19`) montre que ce gestionnaire n'accède à aucune variable en mémoire vive :
> ```avr
> std Y+4, r1    ; r1 = __zero_reg__ (0x00)
> ldi r16, 0x03  ; Taille trame
> ldi r17, 0x00
> rcall 0x13430  ; Envoi trame + CRC
> ```
> Cette valeur **reste systématiquement à zéro quoi qu'il arrive**, même lorsqu'un superviseur Raspberry Pi ou un KitCom officiel est connecté et en cours de session active sur le port série `USART1` (où la commande `RaspberryPiModeOK\r` arme bien `g_kit_com_present = 1` en SRAM `0x0DB0` et répond `KitCom:1` à `KitCom?\r`).  
> Côté BLE, la configuration réseau n'a jamais été finalisée par Legrand (l'application mobile utilise ce `0` pour masquer les écrans de configuration Wi-Fi/Ethernet). Ne jamais utiliser cet opcode pour déduire la présence d'un superviseur ou d'une carte réseau.

#### G. Nom de la Borne (`Type 0x04`, `Opcode 0x09` — `NomBorne`)
* **Request:** `01 04 09 FF 09 06`
* **Response:** `01 04 09 4C 65 67 72 61 6E 64 00 00 00 FF 68 81`
  - Payload: `4C 65 67 72 61 6E 64 00 00 00` (ASCII `"Legrand"`)

#### H. Statut de Charge (`Type 0x04`, `Opcode 0x11` — `EtatCharge`)
* **Request:** `01 04 11 FF 09 0C`
* **Response:** `01 04 11 00 FF B5 08`
  - Payload: `0x00` = `NonConnecte` (Véhicule débranché)

#### I. État Opérationnel Automate (`Type 0x04`, `Opcode 0x15` — `EtatBorne`)
* **Request:** `01 04 15 FF C9 0E`
* **Response:** `01 04 15 01 FF E4 48`
  - Payload: `0x01` = `Actif` / Opérationnel

#### J. Courants de Charge Autorisés (`Type 0x04`, `Opcode 0x16` — `CourantCharge`)
* **Request:** `01 04 16 FF 39 0E`
* **Response:** `01 04 16 10 14 19 20 20 FF D7 1C`
  - Payload: `[0x10, 0x14, 0x19, 0x20, 0x20]` = Calibres autorisés `[16A, 20A, 25A, 32A, 32A]`

#### K. Mode de Charge Actif (`Type 0x04`, `Opcode 0x17` — `ModeCharge`)
* **Request:** `01 04 17 FF A9 0F`
* **Response:** `01 04 17 00 FF B4 E8`
  - Payload: `0x00` = `Permanent` (Charge immédiate à l'enfichage)

#### L. Diagnostic Erreurs Matérielles (`Type 0x04`, `Opcode 0x12` — `ErreurCodification`)
* **Request:** `01 04 12 FF F9 0C`
* **Response:** `01 04 12 00 FF B5 F8`
  - Payload: `0x00` = Aucune erreur

#### M. Procédures de Maintenance Interne (`Opcode 0x23`)
Les opcodes liés au basculement en mode maintenance constructeur ne font pas partie de l'interface opérationnelle de supervision et ne sont pas documentés ici.

#### N. Handling the ATmega Raw USART0 Echo Loop (`0x16AC0`)
If the ATmega USART0 receives any leading byte that is not `0x01` (e.g. modem unsolicited notifications like `+UUBTLC` emitted by u-blox NINA upon BLE connection, or during MCU reboot), it falls into the `0x16AC0` loop and echoes incoming bytes back byte-for-byte.
Robust supervisory clients must:
1. Purge/drain the GATT receive queue for 200–300 ms after establishing the BLE connection before sending the first command.
2. Explicitly reject any received frame whose bytes are strictly identical to the transmitted request (`if rx_frame == tx_frame: ignore`).

---

## ⚡ Service Type 0x03: Firmware Update Services (Notice)

Le Service Type `0x03` est réservé aux procédures de téléversement et de mise à jour de firmware par l'application mobile officielle Legrand :
* `0x01` : Interrogation de conformité du chargeur d'amorçage.
* `0x02` : Déclenchement de la procédure d'arrêt sécurisé avant mise à jour.
* `0x03` : Contrôle de signature d'autorisation constructeur.

> [!NOTE]
> Le projet `greenup-link` ayant pour objectif exclusif la supervision énergétique et le pilotage de charge (EVCC / Home Assistant), les procédures de bas niveau de flashage matériel sont intentionnellement hors du périmètre de ce pilote opérationnel.

---

## 🏭 Service Type 0x05: Factory Calibration & Manufacturing (Notice)

Le Service Type `0x05` regroupe les fonctions industrielles réservées à la ligne de fabrication Legrand (calibrage des shunts de mesure de courant, gravure des identifiants d'usine en EEPROM). Ces registres sont verrouillés en production (`g_production_mode`) et n'ont pas de finalité dans le pilotage opérationnel d'une borne installée.

---

## ⚖️ Technical Comparison: Supervisor (`Serial1`) vs. Bluetooth (`Serial`)

| Feature | Supervisor Port (`Serial1`) | Bluetooth Port (`Serial`) |
| :--- | :--- | :--- |
| **Physical UART** | `USART1` (Pins `PD2`/RXD1, `PD3`/TXD1) | `USART0` (Pins `PE0`/RXD0, `PE1`/TXD0) |
| **Baud Rate** | 115200 bauds 8N1 | 115200 bauds 8N1 |
| **Wire Protocol** | **ASCII text in clear** | **Structured Binary Framed RPC** |
| **Frame Delimiter** | Carriage Return (`\r`, ASCII `0x0D`) | Header `0x01` ... Tail `0xFF` |
| **Error Checking** | None (Raw text) | **CRC-16-IBM / Modbus (`0xA001`)** |
| **Message Paradigm** | **Asynchronous Push (Event-Driven)** | **Half-Duplex Pull (Strict Polling)** |
| **Spontaneous Events** | Yes (`State:X\r`, `CCI:...`, `WT:...`) | **No (Never transmits spontaneously)** |
| **FSM State Query** | String command: `State?\r` | Binary packet: `[0x01, 0x04, 0x15, CRC, 0xFF]` |
| **Set Current Limit** | String command: `CC:16\r` | Binary packet: `[0x01, 0x04, 0x04, 0x10, CRC, 0xFF]` |
| **Start / Stop Charge** | String command: `DOK\r` / `DNOK\r` | Binary packet: `[0x01, 0x04, 0x27, <0/1>, CRC, 0xFF]` |
| **Emergency Unlock** | String command: `Unlock\r` | Binary packet: `[0x01, 0x04, 0x44, CRC, 0xFF]` |
| **Firmware Update** | Handshake constructeur + avrdude | Procédure officielle OTA (Type 0x03) |
