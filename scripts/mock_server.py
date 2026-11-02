#!/usr/bin/env python3
"""
Lightweight mock server to preview and test the Green'Up Link Web UI locally on Windows (or any OS)
without needing a physical serial port or Raspberry Pi.

Supports Live Logs streaming (SSE) and raw command simulation.

Usage:
    python scripts/mock_server.py
    Then open http://localhost:8080 in your browser.
"""

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

# Simulated backend state
state = {
    "serial": {
        "status": "usb_connected",
        "port": "/dev/ttyUSB0",
        "message": "USB serial link active - Station connected",
        "last_rx_secs_ago": 0,
    },
    "voltage": 230.8,
    "current": 16.0,
    "power": 3692.0,
    "energy": 14250.0,
    "greenup_state": "E",
    "iec_state": "C",
    "charge_authorized": True,
    "charge_paused": False,
    "charge_complete": False,
    "tic_mode": "9600",
    "eliot_limit_amps": 16,
    "error_code": "0000",
    "bluetooth_enabled": True,
    "log_level": "INFO",
}

mock_logs = []
log_counter = 0

def add_mock_log(category, level, message):
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
    return {
        "vendor": "Legrand",
        "model": "059001",
        "serial_number": "LEG123456789",
        "hardware_version": "01",
        "firmware_version": "17.77",
        "production_date": "12/20",
        "link_version": "0.5.0-mock",
        "supervision_mode": state.get("supervision_mode", "remote"),
        "serial_connection": state["serial"],
        "is_dual": False,
        "available_connectors": [1],
        "bluetooth_enabled": state["bluetooth_enabled"],
        "mac_address": "AA:BB:CC:DD:EE:FF",
        "capabilities": {
            "phases": 1,
            "max_power_kw": 7.4,
            "max_current_amps": 32,
            "has_schuko": False,
            "charging_points": 1,
            "is_known": True,
            "has_tic": True,
        }
    }

def get_mock_connector():
    is_charging = state["greenup_state"] in ("C", "D", "E") and not state["charge_paused"]
    return {
        "id": 1,
        "status": {
            "ocpp": "Charging" if is_charging else ("SuspendedEVSE" if state["charge_paused"] else ("Preparing" if state["greenup_state"] == "B" else "Available")),
            "iec": state["iec_state"],
            "raw_code": state["greenup_state"],
            "vehicle_connected": state["greenup_state"] in ("B", "C", "D", "E"),
            "charging": is_charging,
            "paused": state["charge_paused"],
            "charge_complete": state["charge_complete"],
            "error_code": state["error_code"],
            "error_description": "No error" if state["error_code"] == "0000" else "Mock hardware error",
        },
        "control": {
            "setpoint_amps": state["eliot_limit_amps"],
            "pilot_limit_amps": state["eliot_limit_amps"],
            "min_allowed_amps": 7,
            "max_hardware_amps": 32,
            "charge_authorized": state["charge_authorized"] and not state["charge_paused"],
            "pilot_voltage": 6 if is_charging else (9 if state["greenup_state"] == "B" else 12),
        },
        "meter": {
            "power_w": state["power"],
            "energy_kwh": round(state["energy"] / 1000.0, 3),
            "energy_total_wh": state["energy"],
            "current_a": state["current"],
            "voltage_v": state["voltage"],
            "voltage_source": state.get("voltage_source", "default"),
            "voltage_ttl_remaining_s": state.get("voltage_ttl_remaining_s", None),
            "frequency_hz": 50.0,
        },
        "grid": {
            "tic_connected": True,
            "tic_mode": "standard",
            "tic_baud_rate": 9600,
        }
    }

def get_mock_snapshot():
    return {
        "station": get_mock_station(),
        "connectors": [get_mock_connector()],
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

mock_saved_sessions = [mock_past_session]

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

    def do_GET(self):
        url = self.path.split("?")[0]

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
            add_mock_log("API", "DEBUG", "🔐 GET /api/auth/status (auth_required: False)")
            self.send_json({"auth_required": False, "authenticated": True})
        elif url == "/api/state":
            if state["iec_state"] == "C" and not state["charge_paused"]:
                state["power"] = state["voltage"] * state["current"]
                state["energy"] += (state["power"] * 2.0 / 3600.0)
            self.send_json(get_mock_snapshot())
        elif url == "/api/station":
            add_mock_log("API", "DEBUG", "🔍 GET /api/station")
            self.send_json(get_mock_station())
        elif url == "/api/connectors":
            self.send_json([get_mock_connector()])
        elif url in ("/api/connectors/1", "/api/connectors/2"):
            self.send_json(get_mock_connector())
        elif url in ("/api/connectors/1/status", "/api/connectors/2/status", "/api/status"):
            self.send_json(get_mock_connector()["status"])
        elif url in ("/api/connectors/1/meter", "/api/connectors/2/meter", "/api/meter"):
            self.send_json(get_mock_connector()["meter"])
        elif url == "/api/mode":
            self.send_json({"mode": state.get("supervision_mode", "remote"), "can_switch": True})
        elif url == "/api/logs":
            add_mock_log("API", "DEBUG", f"📋 GET /api/logs ({len(mock_logs)} entries)")
            self.send_json(mock_logs)
        elif url == "/api/logs/stream":
            add_mock_log("API", "DEBUG", "📡 GET /api/logs/stream (SSE client connected)")
            self.handle_sse_stream()
        elif url == "/api/logs/export":
            self.handle_logs_export()
        elif url == "/api/sessions":
            add_mock_log("API", "DEBUG", f"📊 GET /api/sessions (active: {mock_active_session['session_id']}, saved: {len(mock_saved_sessions)})")
            self.send_json({"active_session": mock_active_session, "sessions": mock_saved_sessions})
        elif url.startswith("/api/sessions/") and url.endswith("/export"):
            parts = url.split("/")
            if len(parts) >= 5:
                session_id = parts[3]
                parsed_url = urllib.parse.urlparse(self.path)
                query_params = urllib.parse.parse_qs(parsed_url.query)
                fmt = query_params.get("format", ["csv"])[0].lower()
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

        if url.startswith("/api/current/"):
            try:
                amps = int(url.split("/")[-1])
                state["eliot_limit_amps"] = amps
                state["current"] = float(amps) if state["charge_authorized"] and not state["charge_paused"] else 0.0
                state["power"] = state["voltage"] * state["current"]
                add_mock_log("API", "INFO", f"⚡ Setpoint changed to: {amps}A")
                add_mock_log("SERIAL_TX", "INFO", f"FM1:{amps}")
                add_mock_log("SERIAL_RX", "INFO", f"FM1:{amps}:OK")
                self.send_json({"status": "success", "message": f"Current limit set to {amps}A"})
            except ValueError:
                self.send_error(400, "Invalid amps value")
        elif url == "/api/charge":
            try:
                data = json.loads(body) if body else {}
                action = data.get("action", "")
                if action == "enable":
                    state["charge_authorized"] = True
                    state["iec_state"] = "C"
                    state["greenup_state"] = "E"
                    state["current"] = float(state["eliot_limit_amps"])
                    add_mock_log("API", "INFO", "⚡ Charge enable requested (T2COK + FM2:0 + DOK)")
                    add_mock_log("SERIAL_TX", "INFO", "T2COK")
                    add_mock_log("SERIAL_RX", "INFO", "T2C:OK")
                elif action == "disable":
                    state["charge_authorized"] = False
                    state["iec_state"] = "B"
                    state["greenup_state"] = "B"
                    state["current"] = 0.0
                    add_mock_log("API", "INFO", "🛑 Charge disable requested (FM2:1 + DNOK)")
                    add_mock_log("SERIAL_TX", "INFO", "DNOK")
                    add_mock_log("SERIAL_RX", "INFO", "D:NOK")
                elif action == "pause":
                    state["charge_paused"] = True
                    state["iec_state"] = "B"
                    state["greenup_state"] = "M"
                    state["current"] = 0.0
                    add_mock_log("API", "INFO", "⏸ Charge pause requested (SBNOK)")
                    add_mock_log("SERIAL_TX", "INFO", "SBNOK")
                    add_mock_log("SERIAL_RX", "INFO", "SB:NOK")
                elif action == "resume":
                    state["charge_paused"] = False
                    state["iec_state"] = "C"
                    state["greenup_state"] = "E"
                    state["current"] = float(state["eliot_limit_amps"])
                    add_mock_log("API", "INFO", "▶ Charge resume requested (SBOK)")
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
        elif url == "/api/tic/refresh":
            add_mock_log("API", "INFO", "🔄 TIC refresh requested (TICTM:1)")
            add_mock_log("SERIAL_TX", "INFO", "TICTM:1")
            add_mock_log("SERIAL_RX", "INFO", "TICTM:1:OK")
            self.send_json({"status": "success"})
        elif url == "/api/logs/level":
            try:
                data = json.loads(body) if body else {}
                new_level = data.get("level", "INFO").upper()
                state["log_level"] = new_level
                add_mock_log("API", "INFO", f"⚙️ Log level set to {new_level}")
                self.send_json({"status": "success", "level": new_level})
            except Exception as e:
                self.send_error(400, str(e))
        elif url == "/api/command":
            cmd = body.strip()
            import re
            if not re.match(r'^[a-zA-Z0-9:?]{1,20}$', cmd):
                add_mock_log("API", "WARN", f"⛔ Rejected invalid raw command: '{cmd}'")
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "status": "error",
                    "error": "Invalid command format. Allowed characters: [a-zA-Z0-9:?], length 1-20 characters."
                }).encode("utf-8"))
                return

            if cmd.lower() == "service":
                add_mock_log("API", "WARN", "⚠️ Warning: ATmega interactive service menu triggered via raw command")
                add_mock_log("SERIAL_TX", "INFO", cmd)
                add_mock_log("SERIAL_RX", "INFO", "---service menu--- Press 0 : Exit")
            else:
                add_mock_log("API", "INFO", f"🔧 Raw command received: {cmd}")
                add_mock_log("SERIAL_TX", "INFO", cmd)
                # Simulated responses
                if cmd == "State?":
                    add_mock_log("SERIAL_RX", "INFO", f"State:{state['greenup_state']}")
                elif cmd == "CC?":
                    add_mock_log("SERIAL_RX", "INFO", "CC:32A")
                elif cmd == "Side?":
                    add_mock_log("SERIAL_RX", "INFO", "Side:Left")
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
                state["voltage"] = v
                state["voltage_source"] = "external"
                state["voltage_ttl_remaining_s"] = 30
                state["power"] = state["voltage"] * state["current"]
                add_mock_log("API", "INFO", f"⚡ External voltage injected: {v}V")
                self.send_json({"status": "success", "message": "External voltage updated"})
            except Exception as e:
                self.send_error(400, str(e))
        elif url == "/api/init":
            add_mock_log("API", "INFO", "🔄 Initialization sequence started (mock)")
            self.send_json({"status": "success", "message": "Initialization sequence started"})
        elif url == "/api/reset":
            add_mock_log("API", "WARN", "⚠️ Hardware restart requested (mock)")
            self.send_json({"status": "success", "message": "ATmega board restart requested"})
        else:
            self.send_json({"status": "success"})

    def do_DELETE(self):
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
                if len(mock_saved_sessions) < initial_count:
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

def main():
    server = http.server.ThreadingHTTPServer(("127.0.0.1", PORT), MockHandler)
    url = f"http://localhost:{PORT}"
    print(f"=======================================================")
    print(f"🚀 Green'Up Link Mock UI Server running at: {url}")
    print(f"Press Ctrl+C to stop.")
    print(f"=======================================================")

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down mock server.")
        server.server_close()

if __name__ == "__main__":
    main()