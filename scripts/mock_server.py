#!/usr/bin/env python3
"""
Lightweight mock server to preview and test the Green'Up Link Web UI locally on Windows (or any OS)
without needing a physical serial port or Raspberry Pi.

Supports Live Logs streaming (SSE) and raw command simulation.

Usage:
    python scripts/mock_server.py
    Then open http://localhost:8080 in your browser.
"""

import argparse
import datetime
import http.server
import io
import json
import math
import os
import pathlib
import sys
import time
import urllib.parse
import webbrowser
import zipfile

# Ensure UTF-8 output on Windows consoles
if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding='utf-8', errors='replace')
        sys.stderr.reconfigure(encoding='utf-8', errors='replace')
    except Exception:
        pass

PORT = 8080
BASE_DIR = pathlib.Path(__file__).parent.parent / "crates" / "greenup-link" / "src"

# Runtime mock configuration (controllable via CLI arguments or URL query params)
mock_config = {
    "dual": False,
    "has_tic": True,
    "auth_required": False,
    "auth_ihm": False,
    "token": "1234",
}

# Simulated backend state per connector (Point 1 / Right and Point 2 / Left)
connectors_state = {
    1: {
        "id": 1,
        "greenup_state": "E",
        "iec_state": "C",
        "charge_authorized": True,
        "charge_paused": False,
        "charge_complete": False,
        "current": 16.0,
        "voltage": 230.8,
        "power": 3692.0,
        "energy": 14250.0,
        "eliot_limit_amps": 16,
        "error_code": "0000",
    },
    2: {
        "id": 2,
        "greenup_state": "B",
        "iec_state": "B",
        "charge_authorized": True,
        "charge_paused": False,
        "charge_complete": False,
        "current": 0.0,
        "voltage": 231.2,
        "power": 0.0,
        "energy": 5120.0,
        "eliot_limit_amps": 32,
        "error_code": "0000",
    },
}

# Reference for point 1 common properties
state = connectors_state[1]
state["serial"] = {
    "status": "usb_connected",
    "port": "/dev/ttyUSB0",
    "message": "USB serial link active - Station connected",
    "last_rx_secs_ago": 0,
}
state["bluetooth_enabled"] = True
state["log_level"] = "INFO"

mock_logs = []
log_counter = 0

def add_mock_log(category, level, message):
    current_level = state.get("log_level", "INFO").upper()
    weights = {"ERROR": 1, "WARN": 2, "INFO": 3, "DEBUG": 4, "TRACE": 5}
    if weights.get(level.upper(), 3) > weights.get(current_level, 3):
        return None
    global log_counter
    log_counter += 1
    now = datetime.datetime.now().strftime("%H:%M:%S.%f")[:-3]
    entry = {
        "id": log_counter,
        "timestamp": now,
        "category": category,
        "level": level,
        "message": message,
    }
    mock_logs.append(entry)
    if len(mock_logs) > 500:
        mock_logs.pop(0)
    return entry

def apply_state(state_letter, target_side=1):
    """Applies a Green'Up state (A, B, C, D, E, F, G, H) to the simulated connector."""
    state_letter = state_letter.upper()
    c = connectors_state.get(target_side, connectors_state[1])
    c["greenup_state"] = state_letter
    if state_letter == "A":
        c["iec_state"] = "A"
        c["current"] = 0.0
        c["power"] = 0.0
        c["charge_authorized"] = True
        c["charge_paused"] = False
        c["charge_complete"] = False
    elif state_letter in ("B", "F"):
        c["iec_state"] = "B"
        c["current"] = 0.0
        c["power"] = 0.0
        c["charge_authorized"] = False
        c["charge_paused"] = False
        c["charge_complete"] = False
    elif state_letter == "C":
        c["iec_state"] = "B"
        c["current"] = 0.0
        c["power"] = 0.0
        c["charge_authorized"] = True
        c["charge_paused"] = False
        c["charge_complete"] = False
    elif state_letter in ("D", "E", "G", "H"):
        c["iec_state"] = "C"
        c["current"] = float(c.get("eliot_limit_amps", 16))
        c["power"] = round(c["voltage"] * c["current"], 1)
        c["charge_authorized"] = True
        c["charge_paused"] = False
        c["charge_complete"] = False
    add_mock_log("API", "INFO", f"🔄 État borne changé en {state_letter} (Point {target_side})")
    add_mock_log("SERIAL_RX", "INFO", f"State:{state_letter}")

# Seed initial logs
add_mock_log("SYS", "INFO", "=== GreenUp Link started in Mock Mode ===")
add_mock_log("SERIAL", "INFO", "Serial port opened: /dev/mock_ttyUSB0 @ 115200 baud")
add_mock_log("SERIAL_TX", "INFO", "RaspberryPiModeOK")
add_mock_log("SERIAL_RX", "INFO", "DIP1:1")
add_mock_log("SERIAL_TX", "INFO", "!GET_REF")
add_mock_log("SERIAL_RX", "INFO", "Ref:059001")
add_mock_log("SERIAL_TX", "INFO", "!GET_SW_VER")
add_mock_log("SERIAL_RX", "INFO", "SwVer:17.77")
add_mock_log("SERIAL_TX", "INFO", "!GET_STATE")
add_mock_log("SERIAL_RX", "INFO", "State:E")
add_mock_log("API", "INFO", "🔍 System ready, telemetry streaming active")
add_mock_log("SYS", "DEBUG", "UART baudrate initialized: 115200 (8N1)")
add_mock_log("SERIAL", "DEBUG", "Task serial_poll started (interval: 100ms)")
add_mock_log("SYS", "DEBUG", "Memory ringbuffer: 500 slots allocated")
add_mock_log("SYS", "DEBUG", "Watchdog timer armed (timeout: 5000ms)")
add_mock_log("SERIAL", "WARN", "⚠️ Retrying frame sync (mock warning)")
add_mock_log("SYS", "ERROR", "❌ Simulated minor error check (mock error)")

def get_mock_station():
    is_dual = mock_config["dual"]
    has_tic = mock_config["has_tic"]
    return {
        "vendor": "Legrand",
        "model": "059002" if is_dual else "059001",
        "serial_number": "LEG123456789",
        "hardware_version": "01",
        "firmware_version": "17.77",
        "production_date": "12/20",
        "link_version": "0.5.0-mock",
        "supervision_mode": state.get("supervision_mode", "remote"),
        "serial_connection": state["serial"],
        "is_dual": is_dual,
        "available_connectors": [1, 2] if is_dual else [1],
        "bluetooth_enabled": state["bluetooth_enabled"],
        "mac_address": "AA:BB:CC:DD:EE:FF",
        "capabilities": {
            "phases": 1,
            "max_power_kw": 7.4,
            "max_current_amps": 32,
            "has_schuko": False,
            "charging_points": 2 if is_dual else 1,
            "is_known": True,
            "has_tic": has_tic,
        }
    }

def get_mock_connector(connector_id=1):
    c_state = connectors_state.get(connector_id, connectors_state[1])
    is_charging = c_state["greenup_state"] in ("D", "E", "G", "H") and not c_state["charge_paused"]
    has_tic = mock_config["has_tic"]

    if is_charging:
        ocpp_status = "Charging"
    elif c_state["charge_paused"]:
        ocpp_status = "SuspendedEVSE"
    elif c_state["greenup_state"] in ("B", "F"):
        ocpp_status = "Preparing"
    elif c_state["greenup_state"] == "C":
        ocpp_status = "SuspendedEV"
    else:
        ocpp_status = "Available"

    if is_charging:
        pilot_v = 6
    elif c_state["greenup_state"] in ("B", "C", "F"):
        pilot_v = 9
    else:
        pilot_v = 12

    return {
        "id": connector_id,
        "status": {
            "ocpp": ocpp_status,
            "iec": c_state["iec_state"],
            "raw_code": c_state["greenup_state"],
            "vehicle_connected": c_state["greenup_state"] in ("B", "C", "D", "E", "F", "G", "H"),
            "charging": is_charging,
            "paused": c_state["charge_paused"],
            "charge_complete": c_state["charge_complete"],
            "error_code": c_state["error_code"],
            "error_description": "No error" if c_state["error_code"] == "0000" else "Mock hardware error",
        },
        "control": {
            "setpoint_amps": c_state["eliot_limit_amps"],
            "pilot_limit_amps": c_state["eliot_limit_amps"],
            "min_allowed_amps": 7,
            "max_hardware_amps": 32,
            "charge_authorized": c_state["charge_authorized"] and not c_state["charge_paused"],
            "pilot_voltage": pilot_v,
        },
        "meter": {
            "power_w": c_state["power"],
            "energy_kwh": round(c_state["energy"] / 1000.0, 3),
            "energy_total_wh": c_state["energy"],
            "current_a": c_state["current"],
            "voltage_v": c_state["voltage"],
            "voltage_source": state.get("voltage_source", "default"),
            "voltage_ttl_remaining_s": state.get("voltage_ttl_remaining_s", None),
            "frequency_hz": 50.0,
        },
        "grid": {
            "tic_connected": has_tic,
            "tic_mode": "standard" if has_tic else "disabled",
            "tic_baud_rate": 9600 if has_tic else None,
        }
    }

def get_mock_snapshot():
    is_dual = mock_config["dual"]
    conns = [get_mock_connector(1), get_mock_connector(2)] if is_dual else [get_mock_connector(1)]
    return {
        "station": get_mock_station(),
        "connectors": conns,
    }

def get_mock_driver_snapshot(side=1):
    is_dual = mock_config["dual"]
    has_tic = mock_config["has_tic"]
    c_state = connectors_state.get(side, connectors_state[1])
    port = "/dev/ttyUSB0" if side == 1 else "/dev/ttyUSB1"
    return {
        "seq": 42,
        "atmega": {
            "kit_com": True,
            "initialized": True,
            "software_version": "V01.17.78",
            "hardware_version": "V02.01.01",
            "serial_number": "015184",
            "reference": "058009" if is_dual else "058001",
            "week_year_production": "42W22",
            "side_raw": str(side),
            "side": side,
            "dip1": "1",
            "dip2": "1",
            "dip3": "0",
            "dip4": "0",
            "state": c_state["greenup_state"],
            "error": c_state["error_code"] or "0000",
            "cp": 6 if c_state["iec_state"] == "C" else (9 if c_state["iec_state"] == "B" else 12),
            "pp": 32,
            "cc_ca": 32,
            "cci": c_state["current"],
            "cci_l2": None,
            "cci_l3": None,
            "cci_updates_count": 42,
            "fcs": c_state["charge_complete"],
            "dom": False,
            "sb": not c_state["charge_paused"],
            "sbf": None,
            "cph": "Mono",
            "cc": c_state["eliot_limit_amps"],
            "ccs": 32,
            "cc_el": c_state["eliot_limit_amps"],
            "cc_co": 32,
            "cc_tic": 32 if has_tic else 0,
            "t2c": c_state["charge_authorized"],
            "dom_2pc": False,
            "t2f": False,
            "dom_2pf": False,
            "pa": False,
            "pafc": False,
            "d": False,
            "dec": 0,
            "fm": "4",
            "fm2": False,
            "cau": False,
            "ocpp_status": "Charging" if c_state["iec_state"] == "C" else "Available",
            "bt": state.get("bluetooth_enabled", True),
            "mac_adr": "0080E1AABBCC",
            "tic_tm": False,
            "tic_test_b": "9600" if has_tic else "0",
            "wt": "WT:14250:CT:01h25m:V:230.8",
            "wt_updates_count": 1,
        },
        "usb": {
            "status": c_state["usb_status"],
            "port": port,
            "baud_rate": 115200,
            "mode": state.get("supervision_mode", "remote"),
            "message": f"Connected to {port} (115200 bauds)",
            "last_rx_secs_ago": 1,
            "init_completed": True,
            "rx_frames_count": 512,
            "tx_commands_count": 128,
        },
    }

# ==============================================================================
# MOCK CHARGING SESSIONS & CURVES GENERATION
# ==============================================================================

MOCK_ACTIVE_SESSION_ID = "session_side1_live"
MOCK_PAST_SESSION_ID = "session_side1_2026-11-02_14-30-00"

def _generate_mock_live_session():
    start = datetime.datetime(2026, 11, 2, 21, 15, 0)
    lines = ["timestamp,elapsed_s,state,current_a,limit_a,voltage_v,power_w,energy_kwh"]
    lines.append(f"{start.strftime('%Y-%m-%d %H:%M:%S')},0,B,0.0,16.0,231.2,0.0,0.000")
    lines.append(f"{(start + datetime.timedelta(seconds=60)).strftime('%Y-%m-%d %H:%M:%S')},60,B,0.0,16.0,231.0,0.0,0.000")
    lines.append(f"{(start + datetime.timedelta(seconds=120)).strftime('%Y-%m-%d %H:%M:%S')},120,C,6.0,16.0,230.8,1385.0,0.005")
    lines.append(f"{(start + datetime.timedelta(seconds=130)).strftime('%Y-%m-%d %H:%M:%S')},130,E,15.8,16.0,230.2,3637.0,0.015")

    total_energy = 0.015
    for t in range(180, 5401, 60):
        dt_h = 60.0 / 3600.0
        v = round(230.5 + 0.6 * math.sin(t / 350.0), 1)
        i = round(16.0 + 0.15 * math.cos(t / 250.0), 1)
        p = round(v * i, 1)
        total_energy += (p * dt_h) / 1000.0
        ts = (start + datetime.timedelta(seconds=t)).strftime("%Y-%m-%d %H:%M:%S")
        lines.append(f"{ts},{t},E,{i:.1f},16.0,{v:.1f},{p:.1f},{total_energy:.3f}")

    csv_text = "\n".join(lines) + "\n"

    txt_text = (
        "================================================================================\n"
        "GREENUP-LINK CHARGING SESSION SUMMARY (IN PROGRESS)\n"
        "================================================================================\n"
        f"Session ID:           {MOCK_ACTIVE_SESSION_ID}\n"
        "Charge Point:         Side 1 (Right / Droit)\n"
        "Start Time:           2026-11-02 21:15:00\n"
        "End Time:             In progress\n"
        "Termination Reason:   Session en cours (State: E)\n"
        "\n"
        "--------------------------------------------------------------------------------\n"
        "DURATIONS\n"
        "--------------------------------------------------------------------------------\n"
        "Total Connected:      1h 30m 00s (5400 s)\n"
        "Active Charging:      1h 27m 50s (5270 s)\n"
        "Idle / Waiting:       2m 10s (130 s)\n"
        "\n"
        "--------------------------------------------------------------------------------\n"
        "ENERGY & ELECTRICAL METRICS\n"
        "--------------------------------------------------------------------------------\n"
        f"Total Energy:         {total_energy:.3f} kWh\n"
        "Phase Detection:      1-phase\n"
        "Plug Type:            Type 2\n"
        "Max Current:          16.20 A\n"
        "Average Current:      16.00 A\n"
        "Max Power:            3735 W\n"
        "Average Active Power: 3680 W\n"
        "\n"
        "--------------------------------------------------------------------------------\n"
        "RAW LEGRAND TICKET\n"
        "--------------------------------------------------------------------------------\n"
        "N/A (Session in progress)\n"
        "================================================================================\n"
    )

    summary = {
        "session_id": MOCK_ACTIVE_SESSION_ID,
        "side": 1,
        "start_time": "2026-11-02 21:15:00",
        "elapsed_seconds": 5400,
        "charging_duration_seconds": 5270,
        "total_energy_kwh": round(total_energy, 2),
        "current_a": 16.0,
        "power_w": 3690.0,
        "max_current_a": 16.2,
        "avg_current_a": 16.0,
        "max_power_w": 3735.0,
        "avg_power_w": 3680.0,
        "state": "E",
        "iec_state": "C",
        "phase": "1-phase",
        "plug_type": "Type 2",
    }
    return summary, csv_text, txt_text

def _generate_mock_past_session():
    start = datetime.datetime(2026, 11, 2, 14, 30, 0)
    lines = ["timestamp,elapsed_s,state,current_a,limit_a,voltage_v,power_w,energy_kwh"]
    lines.append(f"{start.strftime('%Y-%m-%d %H:%M:%S')},0,B,0.0,32.0,231.5,0.0,0.000")
    lines.append(f"{(start + datetime.timedelta(seconds=60)).strftime('%Y-%m-%d %H:%M:%S')},60,B,0.0,32.0,231.4,0.0,0.000")
    lines.append(f"{(start + datetime.timedelta(seconds=120)).strftime('%Y-%m-%d %H:%M:%S')},120,B,0.0,32.0,231.3,0.0,0.000")
    lines.append(f"{(start + datetime.timedelta(seconds=180)).strftime('%Y-%m-%d %H:%M:%S')},180,C,8.0,32.0,230.8,1846.0,0.005")
    lines.append(f"{(start + datetime.timedelta(seconds=190)).strftime('%Y-%m-%d %H:%M:%S')},190,E,20.0,32.0,229.8,4596.0,0.018")
    lines.append(f"{(start + datetime.timedelta(seconds=200)).strftime('%Y-%m-%d %H:%M:%S')},200,E,31.8,32.0,229.4,7295.0,0.038")

    total_energy = 0.038
    last_t = 200

    # Bulk charging plateau (step 120s)
    for t in range(300, 12601, 120):
        dt_h = (t - last_t) / 3600.0
        last_t = t
        v = round(229.6 + 0.7 * math.sin(t / 600.0), 1)
        i = round(32.0 + 0.25 * math.cos(t / 450.0), 1)
        p = round(v * i, 1)
        total_energy += (p * dt_h) / 1000.0
        ts = (start + datetime.timedelta(seconds=t)).strftime("%Y-%m-%d %H:%M:%S")
        lines.append(f"{ts},{t},E,{i:.1f},32.0,{v:.1f},{p:.1f},{total_energy:.3f}")

    # Taper phase near 100% SoC (step 60s)
    for t in range(12660, 14401, 60):
        dt_h = (t - last_t) / 3600.0
        last_t = t
        prog = (t - 12600) / 1800.0
        i = round(max(0.8, 32.0 * ((1.0 - prog) ** 1.4)), 1)
        v = round(230.2 + 1.2 * prog, 1)
        p = round(v * i, 1)
        total_energy += (p * dt_h) / 1000.0
        ts = (start + datetime.timedelta(seconds=t)).strftime("%Y-%m-%d %H:%M:%S")
        lines.append(f"{ts},{t},E,{i:.1f},32.0,{v:.1f},{p:.1f},{total_energy:.3f}")

    # Charge finished (idle / waiting disconnect)
    lines.append(f"{(start + datetime.timedelta(seconds=14460)).strftime('%Y-%m-%d %H:%M:%S')},14460,I,0.0,32.0,231.6,0.0,{total_energy:.3f}")
    lines.append(f"{(start + datetime.timedelta(seconds=14800)).strftime('%Y-%m-%d %H:%M:%S')},14800,I,0.0,32.0,231.6,0.0,{total_energy:.3f}")
    lines.append(f"{(start + datetime.timedelta(seconds=15300)).strftime('%Y-%m-%d %H:%M:%S')},15300,B,0.0,32.0,231.8,0.0,{total_energy:.3f}")
    lines.append(f"{(start + datetime.timedelta(seconds=15312)).strftime('%Y-%m-%d %H:%M:%S')},15312,A,0.0,32.0,231.8,0.0,{total_energy:.3f}")

    csv_text = "\n".join(lines) + "\n"
    csv_bytes = csv_text.encode("utf-8")

    txt_text = (
        "================================================================================\n"
        "GREENUP-LINK CHARGING SESSION SUMMARY\n"
        "================================================================================\n"
        f"Session ID:           {MOCK_PAST_SESSION_ID}\n"
        "Charge Point:         Side 1 (Right / Droit)\n"
        "Start Time:           2026-11-02 14:30:00\n"
        "End Time:             2026-11-02 18:45:12\n"
        "Termination Reason:   Véhicule débranché (State: A)\n"
        "\n"
        "--------------------------------------------------------------------------------\n"
        "DURATIONS\n"
        "--------------------------------------------------------------------------------\n"
        "Total Connected:      4h 15m 12s (15312 s)\n"
        "Active Charging:      3h 56m 40s (14200 s)\n"
        "Idle / Waiting:       18m 32s (1112 s)\n"
        "\n"
        "--------------------------------------------------------------------------------\n"
        "ENERGY & ELECTRICAL METRICS\n"
        "--------------------------------------------------------------------------------\n"
        f"Total Energy:         {total_energy:.3f} kWh\n"
        "Phase Detection:      1-phase\n"
        "Plug Type:            Type 2\n"
        "Max Current:          32.40 A\n"
        "Average Current:      29.80 A\n"
        "Max Power:            7460 W\n"
        "Average Active Power: 6850 W\n"
        "\n"
        "--------------------------------------------------------------------------------\n"
        "RAW LEGRAND TICKET\n"
        "--------------------------------------------------------------------------------\n"
        "WT:00:18:CT:03:56:EVplug:T2:OK\n"
        "================================================================================\n"
    )

    summary = {
        "id": MOCK_PAST_SESSION_ID,
        "side": 1,
        "start_time": "2026-11-02 14:30:00",
        "end_time": "2026-11-02 18:45:12",
        "termination_reason": "Véhicule débranché (State: A)",
        "total_duration_seconds": 15312,
        "charging_duration_seconds": 14200,
        "idle_duration_seconds": 1112,
        "total_energy_kwh": round(total_energy, 2),
        "phase": "1-phase",
        "plug_type": "Type 2",
        "max_current_a": 32.4,
        "avg_current_a": 29.8,
        "max_power_w": 7460.0,
        "avg_power_w": 6850.0,
        "has_csv": True,
        "has_txt": True,
        "csv_size_bytes": len(csv_bytes),
    }
    return summary, csv_text, txt_text

mock_active_session, mock_live_csv, mock_live_txt = _generate_mock_live_session()
mock_past_session, mock_past_csv, mock_past_txt = _generate_mock_past_session()

# Second past session for dual point charge simulation (Point 2 / Left)
mock_past_session_side2 = {
    "id": "session_side2_2026-11-01_19-00-00",
    "side": 2,
    "start_time": "2026-11-01 19:00:00",
    "end_time": "2026-11-01 22:15:30",
    "termination_reason": "Véhicule débranché (State: A)",
    "total_duration_seconds": 11730,
    "charging_duration_seconds": 10800,
    "idle_duration_seconds": 930,
    "total_energy_kwh": 9.85,
    "phase": "1-phase",
    "plug_type": "Type 2",
    "max_current_a": 16.1,
    "avg_current_a": 15.9,
    "max_power_w": 3710.0,
    "avg_power_w": 3660.0,
    "has_csv": True,
    "has_txt": True,
    "csv_size_bytes": 1024,
}

mock_saved_sessions = [mock_past_session]

def get_all_saved_sessions():
    res = list(mock_saved_sessions)
    if mock_config["dual"] and mock_past_session_side2 not in res:
        res.append(mock_past_session_side2)
    return res

class MockHandler(http.server.BaseHTTPRequestHandler):
    def end_headers(self):
        self.send_header("X-Frame-Options", "DENY")
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("Referrer-Policy", "no-referrer")
        super().end_headers()

    def log_message(self, format, *args):
        # Mute high frequency polling logs
        if self.path in ("/api/state", "/api/logs/stream"):
            return
        print(f"[{self.command}] {self.path}")

    def is_authenticated(self):
        if not mock_config["auth_required"]:
            return True
        expected = mock_config.get("token", "")
        if not expected:
            return True

        # 1. Check Authorization header (Bearer or Basic)
        auth_header = self.headers.get("Authorization", "").strip()
        if auth_header.lower().startswith("bearer "):
            candidate = auth_header[7:].strip()
            if candidate == expected:
                return True
        elif auth_header.lower().startswith("basic "):
            import base64
            try:
                b64 = auth_header[6:].strip()
                decoded = base64.b64decode(b64).decode("utf-8", errors="ignore")
                if ":" in decoded:
                    user, pass_ = decoded.split(":", 1)
                    if pass_ == expected or (not pass_ and user == expected):
                        return True
                elif decoded == expected:
                    return True
            except Exception:
                pass

        # 2. Check X-API-Key header
        x_api_key = self.headers.get("X-API-Key", "").strip()
        if x_api_key and x_api_key == expected:
            return True

        # 3. Check query param ?token=
        parsed = urllib.parse.urlparse(self.path)
        qs = urllib.parse.parse_qs(parsed.query)
        if qs.get("token", [""])[0] == expected:
            return True

        return False

    def check_mutation_auth(self):
        """Returns True if mutation is permitted, False if 401 was sent."""
        if not mock_config["auth_required"]:
            return True
        if self.is_authenticated():
            return True
        add_mock_log("API", "WARN", f"⛔ 401 Unauthorized for mutation: {self.path} (token required)")
        self.send_response(401)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.end_headers()
        self.wfile.write(json.dumps({
            "status": "error",
            "error": "Authentification requise pour cette action en mode consultation"
        }).encode("utf-8"))
        return False

    def get_target_side(self):
        parsed = urllib.parse.urlparse(self.path)
        qs = urllib.parse.parse_qs(parsed.query)
        side_val = qs.get("side", ["1"])[0]
        try:
            return int(side_val)
        except ValueError:
            return 1

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        url = parsed.path
        qs = urllib.parse.parse_qs(parsed.query)

        # Allow URL query param overrides on index or any GET
        if "dual" in qs:
            mock_config["dual"] = qs["dual"][0].lower() in ("1", "true", "yes")
            add_mock_log("SYS", "INFO", f"🔀 Dual mode set via URL: {mock_config['dual']}")
        if "no_tic" in qs:
            mock_config["has_tic"] = False
            add_mock_log("SYS", "INFO", "🔀 TIC disabled via URL")
        elif "has_tic" in qs:
            mock_config["has_tic"] = qs["has_tic"][0].lower() in ("1", "true", "yes")
            add_mock_log("SYS", "INFO", f"🔀 TIC mode set via URL: {mock_config['has_tic']}")
        # Auth / API token overrides via URL:
        if "with_token" in qs or "has_token" in qs or "auth_required" in qs:
            val = (qs.get("with_token") or qs.get("has_token") or qs.get("auth_required"))[0].lower()
            mock_config["auth_required"] = val in ("1", "true", "yes")
            add_mock_log("SYS", "INFO", f"🔀 API token requirement set via URL: {mock_config['auth_required']}")
        elif "no_token" in qs or "no_auth" in qs:
            mock_config["auth_required"] = False
            add_mock_log("SYS", "INFO", "🔀 Disabled API token requirement (Open LAN) via URL")

        if "ihm_auth" in qs or "auth_ihm" in qs:
            val = (qs.get("ihm_auth") or qs.get("auth_ihm"))[0].lower()
            mock_config["auth_ihm"] = val in ("1", "true", "yes")
            add_mock_log("SYS", "INFO", f"🔀 IHM Auth mode set via URL: {mock_config['auth_ihm']}")
        elif "kiosk" in qs:
            mock_config["auth_ihm"] = False
            add_mock_log("SYS", "INFO", "🔀 Kiosk mode (ihm_auth=false) set via URL")

        if "set_token" in qs or "server_token" in qs or "mock_token" in qs:
            token_val = (qs.get("set_token") or qs.get("server_token") or qs.get("mock_token"))[0].strip()
            mock_config["token"] = token_val
            mock_config["auth_required"] = True
            add_mock_log("SYS", "INFO", f"🔀 Authorized API token set via URL: '{token_val}'")

        # State override via URL (?state=A, B, C, D, E, F, G, H)
        if "state" in qs:
            st = qs["state"][0].upper()
            if st in ("A", "B", "C", "D", "E", "F", "G", "H"):
                target_side = self.get_target_side()
                apply_state(st, target_side)

        # Mode override via URL (?mode=remote, local)
        if "mode" in qs:
            m = qs["mode"][0].lower()
            if m in ("local", "remote"):
                state["supervision_mode"] = m
                add_mock_log("SYS", "INFO", f"🔀 Station control mode set via URL: {m}")

        # Full auth check if ihm_auth=true
        if mock_config["auth_required"] and mock_config["auth_ihm"]:
            is_public = url in ("/", "/favicon.ico", "/css/style.css", "/api/auth/status") or url.startswith(("/js/", "/locales/"))
            if not is_public and not self.is_authenticated():
                self.send_error(401, "Unauthorized")
                return

        if url == "/":
            self.serve_file(BASE_DIR / "index.html", "text/html; charset=utf-8")
        elif url == "/favicon.ico":
            self.serve_file(BASE_DIR / "web" / "favicon.ico", "image/x-icon")
        elif url == "/css/style.css":
            self.serve_file(BASE_DIR / "web" / "css" / "style.css", "text/css; charset=utf-8")
        elif url.startswith("/js/"):
            filename = url[len("/js/"):]
            self.serve_file(BASE_DIR / "web" / "js" / filename, "application/javascript; charset=utf-8")
        elif url.startswith("/locales/"):
            filename = url[len("/locales/"):]
            self.serve_file(BASE_DIR / "locales" / filename, "application/json; charset=utf-8")
        elif url == "/api/auth/status":
            auth_req = mock_config["auth_required"]
            is_authed = self.is_authenticated()
            auth_ihm = mock_config["auth_ihm"] if auth_req else False
            add_mock_log("API", "DEBUG", f"🔐 GET /api/auth/status (required={auth_req}, authenticated={is_authed}, auth_ihm={auth_ihm})")
            self.send_json({
                "auth_required": auth_req,
                "authenticated": is_authed,
                "auth_ihm": auth_ihm,
            })
        elif url == "/api/state":
            c1 = connectors_state[1]
            if c1["iec_state"] == "C" and not c1["charge_paused"]:
                c1["power"] = c1["voltage"] * c1["current"]
                c1["energy"] += (c1["power"] * 2.0 / 3600.0)
            self.send_json(get_mock_snapshot())
        elif url == "/api/station":
            add_mock_log("API", "DEBUG", "🔍 GET /api/station")
            self.send_json(get_mock_station())
        elif url == "/api/connectors":
            conns = [get_mock_connector(1), get_mock_connector(2)] if mock_config["dual"] else [get_mock_connector(1)]
            self.send_json(conns)
        elif url == "/api/connectors/1":
            self.send_json(get_mock_connector(1))
        elif url == "/api/connectors/2":
            if mock_config["dual"]:
                self.send_json(get_mock_connector(2))
            else:
                self.send_error(404, "Connector 2 not found")
        elif url in ("/api/connectors/1/status", "/api/status"):
            self.send_json(get_mock_connector(1)["status"])
        elif url == "/api/connectors/2/status":
            if mock_config["dual"]:
                self.send_json(get_mock_connector(2)["status"])
            else:
                self.send_error(404, "Connector 2 not found")
        elif url in ("/api/connectors/1/meter", "/api/meter"):
            self.send_json(get_mock_connector(1)["meter"])
        elif url == "/api/connectors/2/meter":
            if mock_config["dual"]:
                self.send_json(get_mock_connector(2)["meter"])
            else:
                self.send_error(404, "Connector 2 not found")
        elif url == "/api/mode":
            self.send_json({"mode": state.get("supervision_mode", "remote"), "can_switch": True})
        elif url == "/api/driver/raw":
            if "side" in qs:
                side = self.get_target_side()
                self.send_json(get_mock_driver_snapshot(side))
            else:
                self.send_json({
                    "point1": get_mock_driver_snapshot(1),
                    "point2": get_mock_driver_snapshot(2) if mock_config["dual"] else None,
                })
        elif url == "/api/logs":
            add_mock_log("API", "DEBUG", f"📋 GET /api/logs ({len(mock_logs)} entries)")
            self.send_json(mock_logs)
        elif url == "/api/logs/stream":
            add_mock_log("API", "DEBUG", "📡 GET /api/logs/stream (SSE client connected)")
            self.handle_sse_stream()
        elif url == "/api/logs/export":
            self.handle_logs_export()
        elif url == "/api/sessions":
            side_filter = qs.get("side", [None])[0]
            saved = get_all_saved_sessions()
            if side_filter:
                try:
                    s_id = int(side_filter)
                    act = mock_active_session if (mock_active_session and mock_active_session.get("side") == s_id) else None
                    saved = [s for s in saved if s.get("side") == s_id]
                except ValueError:
                    act = mock_active_session
            else:
                act = mock_active_session
            add_mock_log("API", "DEBUG", f"📊 GET /api/sessions (active: {act['session_id'] if act else 'None'}, saved: {len(saved)})")
            self.send_json({"active_session": act, "sessions": saved})
        elif url.startswith("/api/sessions/") and url.endswith("/export"):
            parts = url.split("/")
            if len(parts) >= 5:
                session_id = parts[3]
                fmt = qs.get("format", ["csv"])[0].lower()
                self.handle_session_export(session_id, fmt)
            else:
                self.send_error(400, "Bad Request")
        elif url == "/api/logs/level":
            add_mock_log("API", "DEBUG", f"🎚️ GET /api/logs/level → {state.get('log_level', 'INFO')}")
            self.send_json({"level": state.get("log_level", "INFO")})
        else:
            self.send_error(404, "Not Found")

    def handle_logs_export(self):
        now = datetime.datetime.now().strftime("%Y-%m-%d_%H-%M-%S")
        filename = f"greenup_{now}.log"
        add_mock_log("API", "DEBUG", f"💾 GET /api/logs/export ({filename})")
        lines = []
        for entry in mock_logs:
            lines.append(f"[{entry['timestamp']}] [{entry['level']}] [{entry['category']}] {entry['message']}\n")
        content = "".join(lines).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/plain; charset=utf-8")
        self.send_header("Content-Disposition", f'attachment; filename="{filename}"')
        self.send_header("Content-Length", str(len(content)))
        self.end_headers()
        self.wfile.write(content)

    def handle_session_export(self, session_id, fmt):
        if session_id == MOCK_ACTIVE_SESSION_ID:
            csv_text = mock_live_csv
            txt_text = mock_live_txt
        elif any(s["id"] == session_id for s in mock_saved_sessions):
            csv_text = mock_past_csv
            txt_text = mock_past_txt
        else:
            self.send_error(404, f"Session {session_id} not found")
            return

        add_mock_log("API", "DEBUG", f"💾 GET /api/sessions/{session_id}/export?format={fmt}")

        if fmt == "csv":
            content = csv_text.encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/csv; charset=utf-8")
            self.send_header("Content-Disposition", f'attachment; filename="{session_id}.csv"')
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            self.wfile.write(content)
        elif fmt == "txt":
            content = txt_text.encode("utf-8")
            self.send_response(200)
            self.send_header("Content-Type", "text/plain; charset=utf-8")
            self.send_header("Content-Disposition", f'attachment; filename="{session_id}.txt"')
            self.send_header("Content-Length", str(len(content)))
            self.end_headers()
            self.wfile.write(content)
        elif fmt == "zip":
            buf = io.BytesIO()
            with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as z:
                z.writestr(f"{session_id}.csv", csv_text.encode("utf-8"))
                z.writestr(f"{session_id}.txt", txt_text.encode("utf-8"))
            zip_bytes = buf.getvalue()
            self.send_response(200)
            self.send_header("Content-Type", "application/zip")
            self.send_header("Content-Disposition", f'attachment; filename="{session_id}.zip"')
            self.send_header("Content-Length", str(len(zip_bytes)))
            self.end_headers()
            self.wfile.write(zip_bytes)
        else:
            self.send_error(400, f"Unsupported export format: {fmt}")

    def handle_sse_stream(self):
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.send_header("Connection", "keep-alive")
        self.send_header("Access-Control-Allow-Origin", "*")
        self.end_headers()

        last_sent_id = mock_logs[-1]["id"] if mock_logs else 0
        counter = 0

        try:
            while True:
                # Check for new logs in mock_logs
                for entry in list(mock_logs):
                    if entry["id"] > last_sent_id:
                        data = json.dumps(entry)
                        self.wfile.write(f"data: {data}\n\n".encode("utf-8"))
                        self.wfile.flush()
                        last_sent_id = entry["id"]

                counter += 1

                # Periodically generate mock DEBUG log every 1s
                if counter % 2 == 0:
                    debug_messages = [
                        ("SERIAL", "UART poll: rx_buf=0 bytes, tx_buf=0 bytes, status=READY"),
                        ("SYS", "Watchdog keepalive ping ok (task_id=0x04)"),
                        ("SYS", "Internal tick: loop_time=1002us heap_free=42816B"),
                        ("SERIAL", "Frame sync active: crc_ok=1284, crc_err=0"),
                        ("API", "SSE client heartbeat keepalive dispatched"),
                    ]
                    cat, msg = debug_messages[(counter // 2) % len(debug_messages)]
                    entry = add_mock_log(cat, "DEBUG", msg)
                    if entry is not None:
                        data = json.dumps(entry)
                        self.wfile.write(f"data: {data}\n\n".encode("utf-8"))
                        self.wfile.flush()
                        last_sent_id = entry["id"]

                # Periodically generate mock serial telemetry log every ~4s
                if counter % 8 == 0:
                    v = round(state["voltage"] + (0.3 if counter % 16 == 0 else -0.2), 1)
                    i = round(state["current"], 1)
                    msg = f"I:{i} V:{v} State:{state['greenup_state']}"
                    entry = add_mock_log("SERIAL_RX", "INFO", msg)
                    if entry is not None:
                        data = json.dumps(entry)
                        self.wfile.write(f"data: {data}\n\n".encode("utf-8"))
                        self.wfile.flush()
                        last_sent_id = entry["id"]

                time.sleep(0.5)
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError, ConnectionError):
            pass

    def do_POST(self):
        url = self.path.split("?")[0]
        content_len = int(self.headers.get("Content-Length", 0))
        if content_len > 4096:
            add_mock_log("API", "WARN", f"⛔ Request payload exceeded 4 KB limit ({content_len} bytes)")
            self.send_response(413)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({
                "status": "error",
                "error": "Payload too large. Maximum body size is 4 KB."
            }).encode("utf-8"))
            return

        body = self.rfile.read(content_len).decode("utf-8") if content_len > 0 else ""

        # Allow dynamic parameterization of auth status via POST /api/auth/status
        if url == "/api/auth/status":
            try:
                data = json.loads(body) if body else {}
                if "auth_required" in data:
                    mock_config["auth_required"] = bool(data["auth_required"])
                if "with_token" in data or "has_token" in data:
                    mock_config["auth_required"] = bool(data.get("with_token", data.get("has_token")))
                if "auth_ihm" in data:
                    mock_config["auth_ihm"] = bool(data["auth_ihm"])
                if "ihm_auth" in data:
                    mock_config["auth_ihm"] = bool(data["ihm_auth"])
                if "token" in data:
                    mock_config["token"] = str(data["token"])
                add_mock_log("API", "INFO", f"🔐 Reconfigured auth via POST: required={mock_config['auth_required']}, ihm_auth={mock_config['auth_ihm']}, token={mock_config['token']}")
                auth_req = mock_config["auth_required"]
                auth_ihm = mock_config["auth_ihm"] if auth_req else False
                is_authed = self.is_authenticated()
                self.send_json({
                    "status": "success",
                    "auth_required": auth_req,
                    "authenticated": is_authed,
                    "auth_ihm": auth_ihm,
                    "token": mock_config["token"],
                })
                return
            except Exception as e:
                self.send_error(400, f"Invalid JSON: {e}")
                return

        # Check mutation authorization (kiosk mode blocks mutation unless token provided)
        if not self.check_mutation_auth():
            return

        if url.startswith("/api/current/"):
            try:
                amps = int(url.split("/")[-1])
                target_side = self.get_target_side()
                c = connectors_state.get(target_side, connectors_state[1])
                c["eliot_limit_amps"] = amps
                c["current"] = float(amps) if c["charge_authorized"] and not c["charge_paused"] else 0.0
                c["power"] = c["voltage"] * c["current"]
                add_mock_log("API", "INFO", f"⚡ Setpoint changed to: {amps}A (Point {target_side})")
                add_mock_log("SERIAL_TX", "INFO", f"FM1:{amps}")
                add_mock_log("SERIAL_RX", "INFO", f"FM1:{amps}:OK")
                self.send_json({"status": "success", "message": f"Current limit set to {amps}A"})
            except ValueError:
                self.send_error(400, "Invalid amps value")
        elif url == "/api/charge":
            try:
                data = json.loads(body) if body else {}
                action = data.get("action", "")
                target_side = self.get_target_side()
                c = connectors_state.get(target_side, connectors_state[1])
                if action == "enable":
                    c["charge_authorized"] = True
                    c["iec_state"] = "C"
                    c["greenup_state"] = "E"
                    c["current"] = float(c["eliot_limit_amps"])
                    c["power"] = c["voltage"] * c["current"]
                    add_mock_log("API", "INFO", f"⚡ Charge enable requested (Point {target_side})")
                    add_mock_log("SERIAL_TX", "INFO", "T2COK")
                    add_mock_log("SERIAL_RX", "INFO", "T2C:OK")
                elif action == "disable":
                    c["charge_authorized"] = False
                    c["iec_state"] = "B"
                    c["greenup_state"] = "B"
                    c["current"] = 0.0
                    c["power"] = 0.0
                    add_mock_log("API", "INFO", f"🛑 Charge disable requested (Point {target_side})")
                    add_mock_log("SERIAL_TX", "INFO", "DNOK")
                    add_mock_log("SERIAL_RX", "INFO", "D:NOK")
                elif action == "pause":
                    c["charge_paused"] = True
                    c["iec_state"] = "B"
                    c["greenup_state"] = "M"
                    c["current"] = 0.0
                    c["power"] = 0.0
                    add_mock_log("API", "INFO", f"⏸ Charge pause requested (Point {target_side})")
                    add_mock_log("SERIAL_TX", "INFO", "SBNOK")
                    add_mock_log("SERIAL_RX", "INFO", "SB:NOK")
                elif action == "resume":
                    c["charge_paused"] = False
                    c["iec_state"] = "C"
                    c["greenup_state"] = "E"
                    c["current"] = float(c["eliot_limit_amps"])
                    c["power"] = c["voltage"] * c["current"]
                    add_mock_log("API", "INFO", f"▶ Charge resume requested (Point {target_side})")
                    add_mock_log("SERIAL_TX", "INFO", "SBOK")
                    add_mock_log("SERIAL_RX", "INFO", "SB:OK")
                self.send_json({"status": "success"})
            except Exception as e:
                self.send_error(400, str(e))
        elif url == "/api/bluetooth":
            try:
                data = json.loads(body) if body else {}
                state["bluetooth_enabled"] = data.get("enabled", True)
                add_mock_log("API", "INFO", f"🔵 Bluetooth toggled: {state['bluetooth_enabled']}")
                self.send_json({"status": "success"})
            except Exception as e:
                self.send_error(400, str(e))
        elif url in ("/api/tic/refresh", "/api/tic/detect"):
            if not mock_config["has_tic"]:
                self.send_response(400)
                self.send_header("Content-Type", "application/json; charset=utf-8")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "status": "error",
                    "error": "TIC non supporté sur ce modèle de borne"
                }).encode("utf-8"))
                return
            add_mock_log("API", "INFO", "🔄 TIC refresh/test requested (TICTM:1)")
            add_mock_log("SERIAL_TX", "INFO", "TICTM:1")
            add_mock_log("SERIAL_RX", "INFO", "TICTM:1:OK")
            self.send_json({"status": "success", "mode": "standard", "baud_rate": 9600})
        elif url == "/api/logs/level":
            try:
                data = json.loads(body) if body else {}
                new_level = data.get("level", "INFO").upper()
                state["log_level"] = new_level
                add_mock_log("API", "INFO", f"⚙️ Log level set to {new_level}")
                self.send_json({"status": "success", "level": new_level})
            except Exception as e:
                self.send_error(400, str(e))
        elif url == "/api/state":
            try:
                data = json.loads(body) if body else {}
                st = data.get("state", "").upper()
                target_side = data.get("side", self.get_target_side())
                if st in ("A", "B", "C", "D", "E", "F", "G", "H"):
                    apply_state(st, target_side)
                    self.send_json({"status": "success", "state": st, "side": target_side})
                    return
                else:
                    self.send_error(400, "Invalid state (must be A, B, C, D, E, F, G, or H)")
                    return
            except Exception as e:
                self.send_error(400, str(e))
                return
        elif url == "/api/command":
            cmd = body.strip()
            import re
            if not re.match(r'^[a-zA-Z0-9:?=_]{1,20}$', cmd):
                add_mock_log("API", "WARN", f"⛔ Rejected invalid raw command: '{cmd}'")
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "status": "error",
                    "error": "Invalid command format. Allowed characters: [a-zA-Z0-9:?=_], length 1-20 characters."
                }).encode("utf-8"))
                return

            if cmd.lower() == "service":
                add_mock_log("API", "WARN", "⚠️ Warning: ATmega interactive service menu triggered via raw command")
                add_mock_log("SERIAL_TX", "INFO", cmd)
                add_mock_log("SERIAL_RX", "INFO", "---service menu--- Press 0 : Exit")
            else:
                add_mock_log("API", "INFO", f"🔧 Raw command received: {cmd}")
                add_mock_log("SERIAL_TX", "INFO", cmd)
                target_side = self.get_target_side()
                c = connectors_state.get(target_side, connectors_state[1])
                # Simulated responses
                if cmd.upper() in ("STATE=A", "STATE:A", "SET_STATE:A"):
                    apply_state("A", target_side)
                elif cmd.upper() in ("STATE=B", "STATE:B", "SET_STATE:B"):
                    apply_state("B", target_side)
                elif cmd.upper() in ("STATE=C", "STATE:C", "SET_STATE:C"):
                    apply_state("C", target_side)
                elif cmd.upper() in ("STATE=D", "STATE:D", "SET_STATE:D"):
                    apply_state("D", target_side)
                elif cmd.upper() in ("STATE=E", "STATE:E", "SET_STATE:E"):
                    apply_state("E", target_side)
                elif cmd.upper() in ("STATE=F", "STATE:F", "SET_STATE:F"):
                    apply_state("F", target_side)
                elif cmd.upper() in ("STATE=G", "STATE:G", "SET_STATE:G"):
                    apply_state("G", target_side)
                elif cmd.upper() in ("STATE=H", "STATE:H", "SET_STATE:H"):
                    apply_state("H", target_side)
                elif cmd == "State?":
                    add_mock_log("SERIAL_RX", "INFO", f"State:{c['greenup_state']}")
                elif cmd == "CC?":
                    add_mock_log("SERIAL_RX", "INFO", "CC:32A")
                elif cmd == "Side?":
                    add_mock_log("SERIAL_RX", "INFO", f"Side:{'Right' if target_side == 1 else 'Left'}")
                elif cmd == "RaspberryPiModeOK" or cmd == "RPIMODEOK":
                    add_mock_log("SERIAL_RX", "INFO", "DIP1:1")
                else:
                    add_mock_log("SERIAL_RX", "INFO", "ACK")
            self.send_json({"status": "success", "message": "Command sent"})
        elif url == "/api/mode":
            try:
                data = json.loads(body) if body else {}
                new_mode = data.get("mode", "remote").lower()
                if new_mode not in ("local", "remote"):
                    self.send_error(400, "Invalid mode")
                    return
                state["supervision_mode"] = new_mode
                add_mock_log("API", "INFO", f"⚙️ Supervision mode changed to {new_mode}")
                self.send_json({"status": "success", "mode": new_mode, "message": f"Switched to {new_mode} mode."})
            except Exception as e:
                self.send_error(400, str(e))
        elif url == "/api/voltage":
            try:
                data = json.loads(body) if body else {}
                v = float(data.get("voltage", 230.0))
                target_side = self.get_target_side()
                c = connectors_state.get(target_side, connectors_state[1])
                c["voltage"] = v
                c["power"] = c["voltage"] * c["current"]
                state["voltage_source"] = "external"
                state["voltage_ttl_remaining_s"] = 30
                add_mock_log("API", "INFO", f"⚡ External voltage injected: {v}V")
                self.send_json({"status": "success", "message": "External voltage updated"})
            except Exception as e:
                self.send_error(400, str(e))
        elif url == "/api/init":
            add_mock_log("API", "INFO", "🔄 Initialization sequence started (mock)")
            self.send_json({"status": "success", "message": "Initialization sequence started"})
        elif url in ("/api/reset", "/api/board/reset"):
            add_mock_log("API", "WARN", "⚠️ Hardware restart requested (mock)")
            self.send_json({"status": "success", "message": "ATmega board restart requested"})
        else:
            self.send_json({"status": "success"})

    def do_DELETE(self):
        if not self.check_mutation_auth():
            return
        url = self.path.split("?")[0]
        if url.startswith("/api/sessions/"):
            parts = url.split("/")
            if len(parts) >= 4:
                session_id = parts[3]
                if session_id in ("active", MOCK_ACTIVE_SESSION_ID):
                    self.send_response(409)
                    self.send_header("Content-Type", "application/json; charset=utf-8")
                    self.end_headers()
                    self.wfile.write(json.dumps({"error": "Cannot delete currently active charging session"}).encode("utf-8"))
                    return
                global mock_saved_sessions
                initial_count = len(mock_saved_sessions)
                mock_saved_sessions = [s for s in mock_saved_sessions if s["id"] != session_id]
                if len(mock_saved_sessions) < initial_count or session_id == mock_past_session_side2["id"]:
                    add_mock_log("API", "INFO", f"🗑️ Deleted session: {session_id}")
                    self.send_json({"status": "success", "message": f"Session {session_id} deleted"})
                else:
                    self.send_response(404)
                    self.send_header("Content-Type", "application/json; charset=utf-8")
                    self.end_headers()
                    self.wfile.write(json.dumps({"error": f"Session {session_id} not found"}).encode("utf-8"))
            else:
                self.send_error(400, "Bad Request")
        else:
            self.send_error(404, "Not Found")

    def serve_file(self, filepath, content_type):
        if not filepath.exists():
            self.send_error(404, f"File not found: {filepath}")
            return
        with open(filepath, "rb") as f:
            data = f.read()
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-cache, no-store, must-revalidate")
        self.end_headers()
        self.wfile.write(data)

    def send_json(self, data):
        payload = json.dumps(data).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

def parse_args():
    parser = argparse.ArgumentParser(
        description="Serveur mock local de l'IHM Web Green'Up Link",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Exemples d'utilisation :
  python scripts/mock_server.py
      Mode standard : Borne mono point, TIC actif, état E (En charge), sans clé API (Cadenas ouvert ROUGE)

  python scripts/mock_server.py --state A
      Démarre la borne à l'état A (Libre / Déconnectée, câble non branché)

  python scripts/mock_server.py --state B
      Démarre la borne à l'état B (Véhicule branché, attente autorisation borne)

  python scripts/mock_server.py --state C
      Démarre la borne à l'état C (Borne prête et autorisée, attente demande du véhicule)

  python scripts/mock_server.py --state D
      Démarre la borne à l'état D (En charge standard sans ventilation)

  python scripts/mock_server.py --with-token
      Active la clé API sur la borne (clé autorisée par défaut: 1234, écran verrouillé -> Cadenas VERT une fois connecté)

  python scripts/mock_server.py --no-ihm-auth
  python scripts/mock_server.py --kiosk
      Mode consultation libre (ihm_auth=false -> Cadenas ouvert BLEU)
      Permet de naviguer librement. Au clic sur le cadenas ou lors d'une action, la clé 1234 est demandée.

  python scripts/mock_server.py --auth
      Active l'authentification complète avec ihm_auth=true (Écran verrouillé dès le chargement)

  python scripts/mock_server.py --token 1234 --no-ihm-auth
      Définit une clé API personnalisée (ex: 1234) en mode consultation libre

  python scripts/mock_server.py --no-token
      Force le mode sans clé API (Accès libre LAN -> Cadenas ouvert ROUGE)

  python scripts/mock_server.py --dual --no-tic --state A
      Combine toutes les options pour tester les cas avancés !

Bascules dynamiques à chaud (dans l'URL du navigateur) :
  http://localhost:8080/?state=A           # Bascule la borne à l'état A (B, C, D, E)
  http://localhost:8080/?with_token=1      # Active la présence d'une clé API
  http://localhost:8080/?no_token=1        # Désactive la clé API (Open LAN -> Cadenas ROUGE)
  http://localhost:8080/?ihm_auth=false    # Mode consultation libre (Cadenas BLEU)
  http://localhost:8080/?ihm_auth=true     # Mode complet sécurisé
  http://localhost:8080/?set_token=1234    # Définit la clé API autorisée sur le mock
"""
    )
    parser.add_argument("--port", type=int, default=PORT, help=f"Port d'écoute HTTP (défaut: {PORT})")
    parser.add_argument("--dual", action="store_true", help="Simule une borne double point de charge (2 points)")
    parser.add_argument("--no-tic", action="store_true", help="Simule une borne sans support TIC (Linky masqué)")
    parser.add_argument("--state", "-s", type=str.upper, choices=["A", "B", "C", "D", "E", "F", "G", "H"], default="E",
                        help="État Green'Up initial de la borne : A (Déconnectée), B (Attente borne T2), C (Attente véhicule T2), D/E (Charge T2), F (Attente borne Schuko), G/H (Charge Schuko) [défaut: E]")
    parser.add_argument("--state-2", type=str.upper, choices=["A", "B", "C", "D", "E", "F", "G", "H"], default=None,
                        help="État Green'Up initial pour le Point 2 (en mode double point, défaut: B)")
    parser.add_argument("--token", type=str, default=None,
                        help="Clé API autorisée sur la borne (défaut: 1234)")
    parser.add_argument("--with-token", "--has-token", dest="with_token", action="store_true",
                        help="Active une clé API sur la borne (auth_required=true, token par défaut: 1234)")
    parser.add_argument("--no-token", "--without-token", "--no-auth", dest="no_token", action="store_true",
                        help="Désactive la clé API sur la borne (auth_required=false, accès libre LAN, cadenas rouge)")
    parser.add_argument("--no-ihm-auth", "--ihm-auth-false", "--kiosk", dest="no_ihm_auth", action="store_true",
                        help="Active l'authentification avec ihm_auth=false (Mode consultation libre / Kiosk - cadenas bleu)")
    parser.add_argument("--auth", "--auth-required", "--locked", dest="auth_required", action="store_true",
                        help="Active l'authentification complète avec ihm_auth=true (Écran verrouillé)")
    parser.add_argument("--mode", "-m", type=str.lower, choices=["remote", "local"], default="remote",
                        help="Mode de pilotage initial : remote (Actif/Nominal) ou local (Écoute seule/Passif) [défaut: remote]")
    return parser.parse_args()

def main():
    args = parse_args()
    port = args.port
    mock_config["dual"] = args.dual
    mock_config["has_tic"] = not args.no_tic
    state["supervision_mode"] = args.mode

    # Apply initial state
    apply_state(args.state, 1)
    if args.state_2:
        apply_state(args.state_2, 2)

    if args.token is not None:
        mock_config["token"] = args.token
    else:
        mock_config["token"] = "1234"

    if args.no_token:
        mock_config["auth_required"] = False
        mock_config["auth_ihm"] = False
    elif args.no_ihm_auth:
        mock_config["auth_required"] = True
        mock_config["auth_ihm"] = False
    elif args.auth_required:
        mock_config["auth_required"] = True
        mock_config["auth_ihm"] = True
    elif args.with_token or args.token is not None:
        mock_config["auth_required"] = True
        mock_config["auth_ihm"] = True
    else:
        # Default: no API token configured on the station (Open LAN)
        mock_config["auth_required"] = False
        mock_config["auth_ihm"] = False

    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), MockHandler)
    url = f"http://localhost:{port}"

    state_descriptions = {
        "A": "A (Libre / Déconnectée)",
        "B": "B (Connectée - Attente borne)",
        "C": "C (Connectée - Attente véhicule)",
        "D": "D (En charge standard)",
        "E": "E (En charge Green'Up)",
    }
    st1 = connectors_state[1]["greenup_state"]

    print("================================================================================")
    print(f"🚀 Green'Up Link Mock UI Server démarré sur : {url}")
    print("--------------------------------------------------------------------------------")
    print(f"   • Type de borne        : {'Double point de charge [Point 1 + Point 2]' if mock_config['dual'] else 'Mono point de charge'}")
    print(f"   • État initial borne   : {state_descriptions.get(st1, st1)}")
    if mock_config["dual"]:
        st2 = connectors_state[2]["greenup_state"]
        print(f"   • État Point 2 initial : {state_descriptions.get(st2, st2)}")
    print(f"   • Télé-information TIC : {'Activée (Standard 9600)' if mock_config['has_tic'] else 'Désactivée / Non supportée'}")
    if not mock_config["auth_required"]:
        print(f"   • Clé API / Auth       : Aucune clé configurée (Accès libre LAN -> Cadenas ouvert ROUGE)")
    elif not mock_config["auth_ihm"]:
        print(f"   • Clé API / Auth       : Mode consultation libre (ihm_auth=false / Kiosk -> Cadenas ouvert BLEU)")
        print(f"     Clé API autorisée    : {mock_config['token']}")
    else:
        print(f"   • Clé API / Auth       : Authentification complète (ihm_auth=true -> Verrouillé)")
        print(f"     Clé API autorisée    : {mock_config['token']}")
    print("--------------------------------------------------------------------------------")
    print("💡 Raccourcis CLI :")
    print("   python scripts/mock_server.py --state A            # État A (Libre / Déconnectée)")
    print("   python scripts/mock_server.py --state B            # État B (Attente borne)")
    print("   python scripts/mock_server.py --state C            # État C (Attente véhicule)")
    print("   python scripts/mock_server.py --state D            # État D (En charge standard)")
    print("   python scripts/mock_server.py --state E            # État E (En charge Green'Up)")
    print("   python scripts/mock_server.py --no-ihm-auth        # Clé 1234 en consultation libre (Cadenas BLEU)")
    print("   python scripts/mock_server.py --with-token         # Clé 1234 en mode complet verrouillé")
    print("   python scripts/mock_server.py --dual               # Double point")
    print("   python scripts/mock_server.py --no-tic             # Sans TIC")
    print("")
    print("💡 Bascules dynamiques sans redémarrer le serveur (dans l'URL du navigateur) :")
    print(f"   {url}/?state=A         ou {url}/?state=B  ou  {url}/?state=E")
    print(f"   {url}/?with_token=1    ou {url}/?no_token=1")
    print(f"   {url}/?ihm_auth=false  ou {url}/?ihm_auth=true")
    print(f"   {url}/?set_token=1234")
    print(f"   {url}/?dual=1          ou {url}/?dual=0")
    print(f"   {url}/?no_tic=1        ou {url}/?has_tic=1")
    print("================================================================================")
    print("Appuyez sur Ctrl+C pour arrêter le serveur.")

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nArrêt du serveur mock.")
        server.server_close()

if __name__ == "__main__":
    main()