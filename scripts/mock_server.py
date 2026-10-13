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
import json
import os
import pathlib
import sys
import time
import webbrowser

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
add_mock_log("SERIAL_TX", "INFO", "RPIMODEOK")
add_mock_log("SERIAL_RX", "INFO", "RPIMODE:OK")
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

class MockHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        # Mute high frequency polling logs
        if self.path in ("/api/telemetry", "/api/logs/stream"):
            return
        print(f"[{self.command}] {self.path}")

    def do_GET(self):
        url = self.path.split("?")[0]

        if url == "/":
            self.serve_file(BASE_DIR / "index.html", "text/html; charset=utf-8")
        elif url == "/css/style.css":
            self.serve_file(BASE_DIR / "web" / "css" / "style.css", "text/css; charset=utf-8")
        elif url.startswith("/js/"):
            filename = url[len("/js/"):]
            self.serve_file(BASE_DIR / "web" / "js" / filename, "application/javascript; charset=utf-8")
        elif url.startswith("/locales/"):
            filename = url[len("/locales/"):]
            self.serve_file(BASE_DIR / "locales" / filename, "application/json; charset=utf-8")
        elif url == "/api/telemetry":
            if state["iec_state"] == "C" and not state["charge_paused"]:
                state["power"] = state["voltage"] * state["current"]
                state["energy"] += (state["power"] * 2.0 / 3600.0)
            self.send_json(state)
        elif url == "/api/info":
            info = {
                "reference": "059001",
                "hardware_version": "01",
                "software_version": "17.77",
                "serial_number": "LEG123456789",
                "week_year_production": "12/20",
                "bluetooth_enabled": state["bluetooth_enabled"],
                "link_version": "0.3.0-mock",
            }
            self.send_json(info)
        elif url == "/api/tic":
            self.send_json({"mode": "HISTORIQUE", "iinst": 2, "isousc": 30, "ptec": "HC"})
        elif url == "/api/logs":
            self.send_json(mock_logs)
        elif url == "/api/logs/stream":
            self.handle_sse_stream()
        elif url == "/api/logs/export":
            self.handle_logs_export()
        else:
            self.send_error(404, "Not Found")

    def handle_logs_export(self):
        now = datetime.datetime.now().strftime("%Y-%m-%d_%H-%M-%S")
        filename = f"greenup_{now}.log"
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
        elif url == "/api/command":
            cmd = body.strip()
            add_mock_log("API", "INFO", f"🔧 Raw command received: {cmd}")
            add_mock_log("SERIAL_TX", "INFO", cmd)
            # Simulated responses
            if cmd in ("!GET_STATE", "State?"):
                add_mock_log("SERIAL_RX", "INFO", f"State:{state['greenup_state']}")
            elif cmd == "CC?":
                add_mock_log("SERIAL_RX", "INFO", "CC:32A")
            elif cmd == "Side?":
                add_mock_log("SERIAL_RX", "INFO", "Side:Left")
            elif cmd == "!GET_ALL":
                add_mock_log("SERIAL_RX", "INFO", f"State:{state['greenup_state']} I:{state['current']} V:{state['voltage']}")
            elif cmd.startswith("FM1:"):
                add_mock_log("SERIAL_RX", "INFO", f"{cmd}:OK")
            elif cmd == "RPIMODEOK":
                add_mock_log("SERIAL_RX", "INFO", "RPIMODE:OK")
            else:
                add_mock_log("SERIAL_RX", "INFO", "ACK")
            self.send_json({"status": "success", "message": "Command sent"})
        else:
            self.send_json({"status": "success"})

    def serve_file(self, filepath, content_type):
        if not filepath.exists():
            self.send_error(404, f"File not found: {filepath}")
            return
        with open(filepath, "rb") as f:
            data = f.read()
        self.send_response(200)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(data)))
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
