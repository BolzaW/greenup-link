#!/usr/bin/env python3
"""
Lightweight mock server to preview and test the Green'Up Link Web UI locally on Windows (or any OS)
without needing a physical serial port or Raspberry Pi.

Usage:
    python scripts/mock_server.py
    Then open http://localhost:8080 in your browser.
"""

import http.server
import json
import os
import pathlib
import sys
import webbrowser

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

class MockHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        # Clean logging
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
            # Add small random noise to voltage/current if charging
            if state["iec_state"] == "C" and not state["charge_paused"]:
                state["power"] = state["voltage"] * state["current"]
                state["energy"] += (state["power"] * 2.0 / 3600.0)  # increment energy slightly
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
        else:
            self.send_error(404, "Not Found")

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
                print(f"⚡ Setpoint changed to: {amps}A")
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
                elif action == "disable":
                    state["charge_authorized"] = False
                    state["iec_state"] = "B"
                    state["greenup_state"] = "B"
                    state["current"] = 0.0
                elif action == "pause":
                    state["charge_paused"] = True
                    state["iec_state"] = "B"
                    state["greenup_state"] = "M"
                    state["current"] = 0.0
                elif action == "resume":
                    state["charge_paused"] = False
                    state["iec_state"] = "C"
                    state["greenup_state"] = "E"
                    state["current"] = float(state["eliot_limit_amps"])
                print(f"🔄 Charge action applied: {action}")
                self.send_json({"status": "success"})
            except Exception as e:
                self.send_error(400, str(e))
        elif url == "/api/bluetooth":
            try:
                data = json.loads(body) if body else {}
                state["bluetooth_enabled"] = data.get("enabled", True)
                print(f"📡 Bluetooth toggled: {state['bluetooth_enabled']}")
                self.send_json({"status": "success"})
            except Exception as e:
                self.send_error(400, str(e))
        elif url == "/api/tic/refresh":
            print("🔄 TIC refresh requested")
            self.send_json({"status": "success"})
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
    server = http.server.HTTPServer(("127.0.0.1", PORT), MockHandler)
    url = f"http://localhost:{PORT}"
    print(f"=======================================================")
    print(f"🚀 Green'Up Link Mock UI Server running at: {url}")
    print(f"Press Ctrl+C to stop.")
    print(f"=======================================================")
    
    # Try opening the default browser automatically
    try:
        webbrowser.open(url)
    except Exception:
        pass

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down mock server.")
        server.server_close()

if __name__ == "__main__":
    main()
