# Legrand Green'Up Premium — State-Sequence Diagrams

Detailed, firmware-verified state transition diagrams derived from the ATmega power board firmware (`greenup-heart`).  
These diagrams cover the most common **home automation / EMS** scenarios (Type 2, domestic 2P+T socket, three-phase load modulation, TIC Linky load-shedding, stops/suspensions, and hardware faults).

---

## Table of Contents

1. [Type 2 (T2S) — Nominal & Smart-Charging Sequences](#1-type-2-t2s--nominal--smart-charging-sequences)
   - [1.1 Direct Charge (FM2:0) — EV Stops & Unplugs](#11-direct-charge-fm20--ev-stops--unplugs)
   - [1.2 Smart-Charging Gate (FM2:1 + DOK/DNOK)](#12-smart-charging-gate-fm21--dokdnok)
   - [1.3 Deferred Start (Dec:) & 30 s EV Wake-Up](#13-deferred-start-dec--30-s-ev-wake-up)
2. [Stops, Suspensions, Traps & Deep Sleep (T2S)](#2-stops-suspensions-traps--deep-sleep-t2s)
   - [2.1 Clean In-Session Suspension (CAUNOK / CAUOK, No TIC)](#21-clean-in-session-suspension-caunok--cauok-no-tic)
   - [2.2 Local Stop Button (SBNOK / SBOK or Short Press)](#22-local-stop-button-sbnok--sbok-or-short-press)
   - [2.3 Pitfalls: T2CNOK Lockout & Unlock Restart Loop](#23-pitfalls-t2cnok-lockout--unlock-restart-loop)
   - [2.4 Deep Sleep Mode (SOK / SNOK)](#24-deep-sleep-mode-sok--snok)
3. [Domestic Socket (2P+T / Schuko) & T2S Priority](#3-domestic-socket-2pt--schuko--t2s-priority)
   - [3.1 Domestic Direct Charge (FM2:0, No Derogation)](#31-domestic-direct-charge-fm20-no-derogation)
   - [3.2 Domestic Conditioned / Derogation (FM2:1 + DOK/DNOK)](#32-domestic-conditioned--derogation-fm21--dokdnok)
   - [3.3 Domestic Local Stop (SBNOK) & Invalidation (2PCNOK)](#33-domestic-local-stop-sbnok--invalidation-2pcnok)
   - [3.4 Absolute Type 2 Priority over Domestic Socket](#34-absolute-type-2-priority-over-domestic-socket)
4. [Three-Phase Modulation & TIC Linky Load-Shedding](#4-three-phase-modulation--tic-linky-load-shedding)
   - [4.1 Three-Phase Charge & Dynamic Current Modulation (11–22 kW)](#41-three-phase-charge--dynamic-current-modulation-1122-kw)
   - [4.2 Hardware PWM Floor Trap → Overcurrent Reboot (E:0010)](#42-hardware-pwm-floor-trap--overcurrent-reboot-e0010)
   - [4.3 TIC Linky Off-Peak (HP/HC) & Autonomous Load-Shedding](#43-tic-linky-off-peak-hphc--autonomous-load-shedding)
5. [Hardware Faults, Safety Interlocks & Power Outage](#5-hardware-faults-safety-interlocks--power-outage)
   - [5.1 Recoverable Faults (State:R) & Auto-Reboot (State:X)](#51-recoverable-faults-stater--auto-reboot-statex)
   - [5.2 Critical Latched Faults (State:S) — Shunt-Trip MX](#52-critical-latched-faults-states--shunt-trip-mx)
   - [5.3 Control Pilot Short (State:T) & Ventilation (State:N)](#53-control-pilot-short-statet--ventilation-staten)
   - [5.4 Mains Power Outage (State:U / State:V — E:0012)](#54-mains-power-outage-stateu--statev--e0012)

---

## 1. Type 2 (T2S) — Nominal & Smart-Charging Sequences

### 1.1 Direct Charge (`FM2:0`) — EV Stops & Unplugs

Plug-and-charge flow when `FM2:0` (direct mode) is active: the station automatically locks the T2S cable, starts PWM after 4.5 s, closes Contactor 1 when the EV pulls CP to 6 V, streams 1 Hz telemetry (`CPh:Mono`, `CCI:<I1>`), pauses in `State:I` if the EV stops drawing (emitting `FCS:1` after 20 min), and ends the session cleanly on unplug.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C

    state "A : Idle" as A1
    state "B : Plugged" as B1
    state "C : PWM Ready" as C1
    state "E : Charging (Direct)" as E1
    state "I : Paused by EV" as I1
    state "L : Unplugging" as L1

    [*] --> A1
    A1 --> B1 : Plug EV (T2C=1, lock OK)<br/>CCCa:xx, CCS:xx, SBF:1, Start, State:B
    B1 --> C1 : Auto-start after 4.5 s (FM2:0, CAU=1)<br/>PWM on, CC:xx, State:C
    C1 --> E1 : EV draws (CP 9V → 6V ≥ 500 ms)<br/>6 mA test OK, Contactor 1 closes, State:E
    E1 --> E1 : 1 Hz telemetry: CPh:Mono, CCI:I1<br/>RAM modulation: CC:07..32
    E1 --> I1 : EV stops / battery full (CP 6V → 9V)<br/>Contactor 1 opens, State:I
    I1 --> B1 : EV resumes (CP → 6V)<br/>State:B → C → E
    I1 --> L1 : Unplug cable (CP = 12V)<br/>(after 20 min in I: FCS:1), State:L
    L1 --> A1 : SBF:0, WT/CT/EVplug ticket<br/>Unlock T2S, State:A
    A1 --> [*]

    class A1 idle
    class B1,C1,I1 wait
    class E1 charge
    class L1 stop
```

### 1.2 Smart-Charging Gate (`FM2:1` + `DOK`/`DNOK`)

Recommended solar surplus / EMS gating mode: pre-arming `FM2:1` + `DNOK` holds the EV in `State:B` with the cable locked and no PWM until the EMS sends `DOK` (or the user holds the front button). Sending `DNOK` mid-charge ramps down cleanly via `State:W` (≤ 6.1 s) without closing the session. Switching `FM2:0` ↔ `FM2:1` mid-charge transitions between `D` and `E` without dropping Contactor 1.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C

    state "A : Idle (FM2:1)" as A2
    state "B : Held by EMS" as B2
    state "C : PWM Ready" as C2
    state "D : Charging (Cond.)" as D2
    state "E : Charging (Direct)" as E2
    state "W : Ramp-Down" as W2
    state "L : Unplugging" as L2

    [*] --> A2 : Pre-armed FM2:1 + DNOK
    A2 --> B2 : Plug EV (lock OK)<br/>CCCa:xx, CCS:xx, SBF:1, Start, State:B
    B2 --> B2 : Held indefinitely (no PWM)<br/>Waiting for solar surplus / EMS
    B2 --> C2 : EMS enable (DOK → D:1)<br/>or front button long press (EM:XX1)<br/>PWM on, CC:xx, State:C
    C2 --> D2 : EV draws (CP 6V, FM2:1 active)<br/>Contactor 1 closes, State:D
    D2 --> E2 : FM2:0 sent mid-charge<br/>(no contactor drop) State:E
    E2 --> D2 : FM2:1 sent mid-charge (D=1)<br/>(no contactor drop) State:D
    D2 --> W2 : EMS disable (DNOK + FM2:1)<br/>PWM → +12V, wait CP≠6V (≤ 6.1 s), State:W
    W2 --> B2 : Contactor 1 opens, cable locked<br/>Session stays open, SBF:1, State:B
    B2 --> L2 : Unplug cable (CP = 12V), State:L
    L2 --> A2 : SBF:0, WT/EVplug ticket, Unlock, State:A
    A2 --> [*]

    class A2 idle
    class B2,C2 wait
    class D2,E2 charge
    class W2,L2 stop
```

### 1.3 Deferred Start (`Dec:`) & 30 s EV Wake-Up

Deferred timer via serial `Dec:<min>` or front button (3h / 6h / 9h), plus the ATmega automatic wake-up pulse (`FUN_code_008c86`: CP 0 V for 3 s → +12 V for 0.9 s → resume PWM) when an EV stays in `State:C` or `State:I` without drawing.

```mermaid
stateDiagram-v2
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C

    state "B : Delayed (FM2:0)" as B3
    state "C : Waiting EV" as C3
    state "E : Charging" as E3
    state "W : Ramp-Down" as W3

    [*] --> B3 : EV plugged, Dec:180 / 360 / 540<br/>(or front button hold 3h/6h/9h)
    B3 --> B3 : Countdown running (no PWM)
    B3 --> C3 : Timer expires (or Dec:0 / T2FOK)<br/>Dec:0, CC:xx, State:C
    C3 --> C3 : EV asleep > 30 s → Wake-up pulse<br/>CP 0V (3 s) → +12V (0.9 s) → PWM (CC:xx)
    C3 --> E3 : EV wakes up (CP 6V)<br/>Contactor 1 closes, State:E
    E3 --> W3 : Dec:xxx armed mid-charge (T2F=0)<br/>PWM → +12V, wait CP≠6V (≤ 6.1 s), State:W
    W3 --> B3 : Contactor 1 opens, State:B

    class B3,C3 wait
    class E3 charge
    class W3 stop
```

---

## 2. Stops, Suspensions, Traps & Deep Sleep (T2S)

### 2.1 Clean In-Session Suspension (`CAUNOK` / `CAUOK`, No TIC)

Temporary suspension from the EMS (`CAUNOK`) without closing the energy session (`WT`/`CT` ticket) and without EEPROM wear (unlike `FM2:` writes). If paused while the EV was already in `State:I`, the ATmega moves to `State:J` under `FM2:1` (or `State:B` under `FM2:0`).

```mermaid
stateDiagram-v2
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C

    state "D/E : Charging" as DE1
    state "W : Ramp-Down" as W1
    state "B : Suspended (Locked)" as B1
    state "I : Paused by EV" as I1
    state "J : Suspended from I" as J1
    state "C : PWM Ready" as C1

    [*] --> DE1
    DE1 --> W1 : CAUNOK (CAU:0)<br/>PWM → +12V, wait CP≠6V (≤ 6.1 s), State:W
    W1 --> B1 : Contactor 1 opens, cable stays locked<br/>No WT/CT ticket, no EEPROM write, State:B
    B1 --> C1 : CAUOK (CAU:1)<br/>CC:xx, State:C
    C1 --> DE1 : EV resumes (CP 6V), State:D/E

    DE1 --> I1 : EV stops (CP 9V), State:I
    I1 --> J1 : CAUNOK under FM2:1<br/>PWM → +12V, State:J
    J1 --> I1 : CAUOK (CAU:1)<br/>CC:xx, State:I
    I1 --> B1 : CAUNOK under FM2:0, State:B

    class B1,C1,I1,J1 wait
    class DE1 charge
    class W1 stop
```

### 2.2 Local Stop Button (`SBNOK` / `SBOK` or Short Press)

Pressing the Stop button (or sending `SBNOK`) closes the active session (`WT`/`CT`/`EVplug`), unlocks the T2S cable, and enters the stable paused `State:M` until the cable is unplugged or `SBOK` / a short press re-arms the station.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C

    state "D/E : Charging" as DE2
    state "W : Ramp-Down" as W2
    state "M : Local Stop (Unlocked)" as M2
    state "A : Idle" as A2
    state "B : New Session" as B2

    [*] --> DE2
    DE2 --> W2 : SBNOK (SB:0) or short press (SBF:0)<br/>PWM → +12V, wait CP≠6V (≤ 6.1 s), State:W
    W2 --> M2 : Contactor 1 opens, closes session<br/>WT/CT/EVplug ticket, Unlocks T2S, State:M
    M2 --> M2 : Stable paused state<br/>(cable unlocked, no auto-restart)
    M2 --> A2 : Unplug cable (CP = 12V), State:A
    M2 --> A2 : SBOK (SB:1) or short press (SBF:1), State:A
    A2 --> B2 : Cable still plugged (or replugged)<br/>Re-locks T2S, SBF:1, Start, State:B

    class A2 idle
    class B2 wait
    class DE2 charge
    class W2,M2 stop
```

### 2.3 Pitfalls: `T2CNOK` Lockout & `Unlock` Restart Loop

Two common supervisor integration traps observed in firmware:
1. **`T2CNOK` (`T2C:0`)**: forces immediate transition through `State:M` to `State:A`, leaving the FSM blind to physical plug/unplug events until `T2COK` is sent.
2. **`Unlock`**: closes the session, unlocks the cable, and returns to `State:A` — if the cable is still plugged and `T2C=1`, the board immediately re-locks and starts a new session (`B → C → E`).

```mermaid
stateDiagram-v2
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C
    classDef warn fill:#FFFDE7,stroke:#F9A825,color:#F57F17

    state "E : Charging" as E3
    state "W : Ramp-Down" as W3
    state "M : Transient" as M3
    state "A : Frozen (T2C=0)" as A3_frozen
    state "A : Idle (T2C=1)" as A3_loop
    state "B : Auto-Restart" as B3

    [*] --> E3
    E3 --> W3 : T2CNOK (T2C:0), State:W
    W3 --> M3 : Opens contactor, WT/CT, unlocks, State:M
    M3 --> A3_frozen : T2C=0 forces immediate exit, State:A
    A3_frozen --> A3_frozen : FSM blind to cable plug/unplug<br/>until T2COK (T2C:1)

    E3 --> A3_loop : Unlock command<br/>State:W → WT/CT → Unlock → State:A
    A3_loop --> B3 : Cable still plugged & T2C=1<br/>Immediately re-locks & restarts!<br/>SBF:1, Start, State:B → C → E

    class E3 charge
    class W3,M3 stop
    class A3_frozen,A3_loop warn
    class B3 wait
```

### 2.4 Deep Sleep Mode (`SOK` / `SNOK`)

Entering deep sleep (`SOK` → `Slp:1`) cleanly ramps down any active session, unlocks the T2S socket, and places the board into low-power `State:Y` (dim LED pulse every 5 s) until `SNOK` (`Slp:0`) restores `State:A`.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C

    state "A/B/C/F : Waiting" as Idle4
    state "D/E : Charging" as DE4
    state "W : Ramp-Down" as W4
    state "Y : Deep Sleep" as Y4
    state "A : Idle" as A4

    Idle4 --> Y4 : SOK (Slp:1)<br/>Closes session, unlocks T2S, State:Y
    DE4 --> W4 : SOK (Slp:1)<br/>PWM → +12V (≤ 6.1 s), State:W
    W4 --> Y4 : Contactor 1 opens, WT/CT<br/>Unlocks T2S, State:Y
    Y4 --> Y4 : Low-power standby<br/>(dim LED pulse every 5 s)
    Y4 --> A4 : SNOK (Slp:0), State:A

    class A4 idle
    class Idle4 wait
    class DE4 charge
    class W4,Y4 stop
```

---

## 3. Domestic Socket (2P+T / Schuko) & T2S Priority

### 3.1 Domestic Direct Charge (`FM2:0`, No Derogation)

Inserting a 2P+T plug closes the mechanical detection micro-switch (`DOM=1`), entering `State:F` and automatically energizing Contactor 2 (16 A) into `State:H` after 4.5 s (provided `2PC=1` and the 6 mA DC leakage test passes).

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100

    state "A : Idle" as A1
    state "F : Schuko Plugged" as F1
    state "H : Domestic Charging" as H1

    [*] --> A1
    A1 --> F1 : Insert Schuko plug (DOM=1, 2PC=1)<br/>SBF:1, Start, State:F
    F1 --> H1 : After 4.5 s (or Dec expiry → Dec:0)<br/>6 mA test OK, Contactor 2 closes (16 A), State:H
    H1 --> H1 : 1 Hz telemetry (Phase 1 CT)<br/>CPh:Mono, CCI:I1
    H1 --> A1 : Unplug Schuko (DOM=0)<br/>Contactor 2 opens, WT/CT/DOMplug, State:A, SBF:0
    A1 --> [*]

    class A1 idle
    class F1 wait
    class H1 charge
```

### 3.2 Domestic Conditioned / Derogation (`FM2:1` + `DOK`/`DNOK`)

Under `FM2:1` (or TIC Peak Hours), inserting a Schuko plug holds Contactor 2 open in `State:F` until authorized by EMS (`DOK`), external dry contact, or front button long press (`State:G`). Sending `2PFOK` forces an immediate transition into `State:H`, bypassing `FM2:1`, `CAU`, and `Dec`.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100

    state "A : Idle (FM2:1)" as A2
    state "F : Held by EMS / HP" as F2
    state "G : Domestic Charging (Cond.)" as G2
    state "H : Domestic Charging (Forced)" as H2

    [*] --> A2 : Pre-armed FM2:1 + DNOK
    A2 --> F2 : Insert Schuko (DOM=1)<br/>SBF:1, Start, State:F
    F2 --> F2 : Held indefinitely (Contactor 2 open)
    F2 --> G2 : DOK (D:1), Contact 1 closes (EM:X1X)<br/>or button long press (EM:XX1)<br/>Contactor 2 closes, State:G
    G2 --> F2 : DNOK (D:0) or CAUNOK (Contact 1 open)<br/>Contactor 2 opens, session stays open, State:F
    F2 --> H2 : Manual force 2PFOK (2PF:1)<br/>Bypasses FM2:1 / CAU / Dec, Contactor 2 closes, State:H
    F2 --> A2 : Unplug Schuko (DOM=0)<br/>WT/DOMplug, SBF:0, State:A
    G2 --> A2 : Unplug Schuko (DOM=0)<br/>Contactor 2 opens, WT/DOMplug, State:A, SBF:0

    class A2 idle
    class F2 wait
    class G2,H2 charge
```

### 3.3 Domestic Local Stop (`SBNOK`) & Invalidation (`2PCNOK`)

Pressing the Stop button during a domestic charge drops Contactor 2, closes the `DOMplug` session, and parks in `State:P` as long as the plug remains inserted. Disabling the domestic socket via `2PCNOK` opens Contactor 2 and returns to `State:A`, ignoring `DOM` until `2PCOK`.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C

    state "G/H : Domestic Charging" as GH3
    state "P : Local Stop (Dom)" as P3
    state "A : Idle" as A3

    [*] --> GH3
    GH3 --> P3 : SBNOK (SB:0) or short press (SBF:0)<br/>Contactor 2 opens, WT/CT/DOMplug, State:P
    P3 --> P3 : Stable paused state (DOM=1)
    P3 --> A3 : Unplug Schuko (DOM=0) or SBOK (SB:1)<br/>State:A (restarts in F if still plugged on SBOK)
    GH3 --> A3 : 2PCNOK (2PC:0)<br/>Contactor 2 opens, WT/CT → P → A<br/>(DOM ignored until 2PCOK)

    class A3 idle
    class GH3 charge
    class P3 stop
```

### 3.4 Absolute Type 2 Priority over Domestic Socket

Because the station shares internal feed constraints, plugging an EV into the Type 2 socket immediately preempts any active 2P+T domestic session (dropping Contactor 2). Once the EV finishes (`State:I`), the firmware automatically hands control back to the still-plugged domestic socket (`State:F` → `G/H`).

```mermaid
stateDiagram-v2
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100

    state "G/H : Domestic Charging" as GH4
    state "B : Preempted by T2S" as B4
    state "D/E : EV Charging" as DE4
    state "I : EV Paused / Full" as I4
    state "F : Domestic Resumes" as F4

    [*] --> GH4 : Schuko active (DOM=1)
    GH4 --> B4 : Plug EV into T2S (T2C=1, lock OK)<br/>Contactor 2 opens immediately!<br/>Domestic session paused, State:B
    B4 --> DE4 : Authorization OK → C → D/E<br/>EV charges with priority
    DE4 --> I4 : EV stops / full (CP 9V), State:I
    I4 --> F4 : DOM still plugged (DOM=1, 2PC=1, CAU=1)<br/>Auto-switch back to Domestic, State:F
    F4 --> GH4 : Contactor 2 closes, State:G/H<br/>(unplugging T2S unlocks & emits EVplug silently)

    class B4,I4,F4 wait
    class GH4,DE4 charge
```

---

## 4. Three-Phase Modulation & TIC Linky Load-Shedding

### 4.1 Three-Phase Charge & Dynamic Current Modulation (11–22 kW)

On a three-phase board (`g_flag_02fd=0`), the effective setpoint is `min(CCS, CCCa, CCEl, CCCo, CCTIC)`. The firmware detects whether a three-phase or single-phase EV is drawing (`CPh:Tri` vs `CPh:Mono`, streaming `CCI:<I1>;<I2>;<I3>` at 1 Hz) and dynamically applies RAM-only `CC:xx` updates without writing EEPROM.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100

    state "A : Idle (3-Phase Board)" as A1
    state "B : Plugged" as B1
    state "C : PWM Ready" as C1
    state "D/E : Charging (3-Ph)" as DE1

    [*] --> A1 : Station 3-phase (g_flag_02fd=0)
    A1 --> B1 : Plug EV (lock OK)<br/>CCCa:32, CCS:32, SBF:1, Start, State:B
    B1 --> C1 : Authorized<br/>Eff = min(CCS, CCCa, CCEl, CCCo, CCTIC)<br/>CC:xx, State:C
    C1 --> DE1 : EV draws (CP 6V ≥ 500 ms)<br/>Contactor 1 closes, State:D/E
    DE1 --> DE1 : 3-phase EV (I1, I2, I3 > 0): CPh:Tri (60 s)<br/>CCI: I1 / I2 / I3 (1 s), PWM floor 22% (13.2 A)
    DE1 --> DE1 : EMS sends CC:13..32 (RAM only, 0 EEPROM)<br/>Immediate PWM update, echoes CC:eff
    DE1 --> DE1 : 1-phase EV on 3-phase board (I2=I3=0)<br/>CPh:Mono, PWM floor drops to 12% (7.2 A)

    class A1 idle
    class B1,C1 wait
    class DE1 charge
```

### 4.2 Hardware PWM Floor Trap → Overcurrent Reboot (`E:0010`)

> [!CAUTION]
> **Critical Three-Phase Limit:** Because the hardware clamps the three-phase PWM duty cycle at **22% (13.2 A)** while comparing measured phase current against `1.2 × CC`, sending `CC:11` (or lower) in three-phase causes `13.2 A ≥ 1.2 × 11 A` → `E:0010` after 8 seconds → forced watchdog reboot! Keep `CC ≥ 13 A` in three-phase (and `CC ≥ 7 A` in single-phase, where the hardware floor is 12% = 7.2 A).

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef fault fill:#FFEBEE,stroke:#C62828,color:#B71C1C

    state "D/E : Charging" as DE2
    state "R : Overcurrent Fault" as R2
    state "X : Watchdog Reboot" as X2
    state "A : Idle (Post-Reboot)" as A2

    [*] --> DE2
    DE2 --> DE2 : Setpoint too low:<br/>- 3-phase: CC ≤ 11 A (PWM clamped 13.2 A)<br/>- 1-phase: CC ≤ 5 A (PWM clamped 7.2 A)<br/>Any phase I ≥ 1.2 × CC for 8 s
    DE2 --> R2 : E:0010, State:R<br/>PWM → +12V (≤ 500 ms), Contactor 1 opens<br/>WT/CT/EVplug ticket, unlocks T2S
    R2 --> X2 : After 5 s (fault_count ≤ 3)<br/>State:X (WDT reset)
    X2 --> A2 : MCU reboots → State:A<br/>(CCCo reset to CCS)

    class A2 idle
    class DE2 charge
    class R2,X2 fault
```

### 4.3 TIC Linky Off-Peak (`HP/HC`) & Autonomous Load-Shedding

When a Linky TIC meter is wired directly to the board, Peak Hours (`HP`) hold `CAU_t2s=0` in `State:B` (overridable by `DOK`), while dynamic overload (`≥ 125% Isub`) or insufficient `CCTIC` (< 8 A 1-ph / < 13 A 3-ph) triggers load-shedding with a **10-minute mandatory penalty** before resuming.

```mermaid
stateDiagram-v2
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C

    state "B : Waiting Off-Peak / TIC" as B3
    state "C : PWM Ready (CCTIC)" as C3
    state "E/D : Charging (TIC)" as ED3
    state "W : Ramp-Down" as W3
    state "I : Paused by EV" as I3
    state "J : Shed from I" as J3

    [*] --> B3 : EV plugged during Peak Hours (HP)<br/>TIC sets CAU_t2s=0, held in State:B
    B3 --> C3 : Linky switches to Off-Peak (HC)<br/>OR EMS sends DOK (overrides HP only)<br/>& CCTIC ≥ 8 A (1-ph) / ≥ 13 A (3-ph), State:C
    C3 --> ED3 : EV draws (CP 6V), State:E (or D if FM2:1)<br/>CCTIC refreshed every 10 s (0.85 × Isub)
    ED3 --> W3 : Peak Hours return OR overload (≥ 125%)<br/>OR CCTIC below min threshold, State:W (≤ 6.1 s)
    W3 --> B3 : Contactor 1 opens, State:B<br/>(Load-shedding enforces 10-min penalty<br/>before allowing B → C resume)
    ED3 --> I3 : EV stops (CP 9V), State:I
    I3 --> J3 : Peak Hours return / TIC shed<br/>PWM → +12V, State:J
    J3 --> I3 : HC returns & 10-min penalty elapsed<br/>CC:CCTIC, State:I

    class B3,C3,I3,J3 wait
    class ED3 charge
    class W3 stop
```

---

## 5. Hardware Faults, Safety Interlocks & Power Outage

### 5.1 Recoverable Faults (`State:R`) & Auto-Reboot (`State:X`)

Lock actuator failure (`E:0001`/`E:0002`), Contactor 1 failure to close (`E:0008`), or 8 s overcurrent (`E:0010`/`E:0011`) enter `State:R` and trigger a watchdog reset (`State:X` → `State:A`) after 5 s (latched after 4 consecutive faults). Missing EV diode (`E:0009`) clears as soon as the cable is unplugged without rebooting.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef fault fill:#FFEBEE,stroke:#C62828,color:#B71C1C

    state "Active / Lock / Close" as Op1
    state "C : Waiting EV" as C1
    state "R : Fault" as R1
    state "X : WDT Reboot" as X1
    state "A : Idle" as A1

    Op1 --> R1 : Lock/Unlock fail E:0001 / E:0002<br/>Contactor 1 close fail E:0008<br/>Overcurrent 8 s E:0010 (T2S) / E:0011 (Dom ≥ 19.2 A)<br/>Opens contactor, WT/CT, unlocks T2S, State:R
    R1 --> X1 : After 5 s (if fault_count ≤ 3)<br/>or front button held 5 s, State:X
    X1 --> A1 : Watchdog reboot → State:A<br/>(Latched after 4 consecutive faults)

    C1 --> R1 : Missing EV diode detected<br/>E:0009, unlocks T2S, State:R
    R1 --> A1 : (E:0009 only) EV cable unplugged<br/>E:0000, State:A (no reboot)

    class A1 idle
    class C1 wait
    class Op1 charge
    class R1,X1 fault
```

### 5.2 Critical Latched Faults (`State:S`) — Shunt-Trip MX

Welded/stuck Contactor 1 (`E:0007`) or 6 mA DC residual current / self-test failure (`E:0015`) immediately fires a 100 ms pulse on the external MX shunt-trip coil to trip the upstream circuit breaker and latches `State:S` until full power cycle.

```mermaid
stateDiagram-v2
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef fault fill:#FFEBEE,stroke:#C62828,color:#B71C1C

    state "D/E/G/H or Check" as Op2
    state "S : Latched Critical Fault" as S2_fault

    Op2 --> S2_fault : Contactor 1 welded/open fail (E:0007)<br/>or 6 mA DC leakage / self-test fail (E:0015)<br/>100 ms MX shunt-trip pulse, opens contactors<br/>Unlocks T2S, SBF:0, State:S
    S2_fault --> S2_fault : Closes sessions, PWM = 0<br/>Remains latched until breaker reset / power cycle

    class Op2 charge
    class S2_fault fault
```

### 5.3 Control Pilot Short (`State:T`) & Ventilation (`State:N`)

- **CP Short (0 V):** Enters `State:T` (`E:0003`); clears back to `State:A` if unplugging the cable removes the short, or latches `State:S` (`E:0004`) if CP stays at 0 V with no cable plugged (internal board fault).
- **Ventilation (3 V / IEC State D):** Unsupported indoors — ramps down via `State:W`, holds PWM at +12 V in `State:N`, and returns to `State:B` (or `State:F`) once CP leaves 3 V.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef stop fill:#F3E5F5,stroke:#6A1B9A,color:#4A148C
    classDef fault fill:#FFEBEE,stroke:#C62828,color:#B71C1C

    state "Any State" as Any3
    state "T : CP Short (0 V)" as T3
    state "S : Board CP Fault" as S3_fault
    state "A : Idle" as A3
    state "C/D/E/I/J" as EV3
    state "N : Ventilation (3 V)" as N3
    state "B : Plugged" as B3

    Any3 --> T3 : Control Pilot = 0 V<br/>Opens active contactor, unlocks T2S<br/>E:0003, State:T
    T3 --> A3 : Short clears (cable unplugged, CP ≠ 0 V)<br/>E:0000, State:A
    T3 --> S3_fault : Cable removed yet CP still 0 V<br/>(internal board short) E:0004, State:S (latched)

    EV3 --> N3 : EV requests ventilation (CP = 3 V)<br/>(from D/E: via State:W ≤ 6.1 s, opens C1)<br/>PWM → +12V, State:N
    N3 --> B3 : EV leaves CP = 3 V<br/>State:B (or State:F if DOM=1)

    class A3 idle
    class Any3,B3 wait
    class EV3 charge
    class N3 stop
    class T3,S3_fault fault
```

### 5.4 Mains Power Outage (`State:U` / `State:V` — `E:0012`)

After ≥ 400 ms loss of 230 V zero-crossing, the ATmega uses its capacitor reserve to emit `E:0012`, gracefully ramp down active sessions via `State:U` → `State:V`, unlock the T2S cable, log the `WT`/`CT` ticket, and perform a clean software reset once mains returns.

```mermaid
stateDiagram-v2
    classDef idle fill:#E8F5E9,stroke:#2E7D32,color:#1B5E20
    classDef wait fill:#E3F2FD,stroke:#1565C0,color:#0D47A1
    classDef charge fill:#FFF3E0,stroke:#EF6C00,color:#E65100
    classDef fault fill:#FFEBEE,stroke:#C62828,color:#B71C1C

    state "C/D/E/F/G/H/I : Active" as Act4
    state "A/B/J/M/P/Y : Idle/Wait" as Idle4
    state "U : Power Loss (Ramp)" as U4
    state "V : Fatal Power Loss" as V4
    state "A : Rebooted" as A4

    Act4 --> U4 : Mains lost ≥ 400 ms, E:0012, State:U<br/>PWM → +12V, wait CP≠6V (≤ 6.1 s), opens C1
    U4 --> V4 : State:V
    Idle4 --> V4 : Mains lost ≥ 400 ms, E:0012, State:V
    V4 --> V4 : Opens C2, unlocks T2S (cap. reserve)<br/>WT/CT ticket, low-power wait for 230 V
    V4 --> A4 : 230 V mains restored<br/>Software reset (jmp 0x0000) → State:A

    class A4 idle
    class Idle4 wait
    class Act4 charge
    class U4,V4 fault
```
