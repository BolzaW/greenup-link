#!/bin/bash
set -e

# ==============================================================================
# Green'Up Link - Automated Installation Script for Raspberry Pi
# ==============================================================================

echo "================================================="
echo "       Green'Up Link - Installer v0.3.0          "
echo "================================================="

# 1. Require root privileges
if [ "$EUID" -ne 0 ]; then
    echo "❌ Error: This script must be run as root."
    echo "   Please re-run with: sudo $0"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="/home/pi/greenup"
TARGET_BIN="$TARGET_DIR/greenup-link"

# 2. Stop and kill legacy Legrand services
echo ""
echo "[1/5] Stopping legacy Legrand services (Tomcat9 & Java daemon)..."
systemctl stop tomcat9 2>/dev/null || true
systemctl disable tomcat9 2>/dev/null || true
pkill -9 -f java 2>/dev/null || true
pkill -9 -f tomcat 2>/dev/null || true
echo "  -> Legacy Java and Tomcat processes stopped."

# 3. Disable Legrand init.d startup script
echo ""
echo "[2/5] Disabling Legrand init.d startup script..."
INIT_SCRIPT="/etc/init.d/CommunicationArduinoRasp"
if [ -f "$INIT_SCRIPT" ]; then
    if [ ! -f "$INIT_SCRIPT.old" ]; then
        cp -p "$INIT_SCRIPT" "$INIT_SCRIPT.old"
        echo "  -> Backup created: $INIT_SCRIPT.old"
    fi
    cat << 'EOF' > "$INIT_SCRIPT"
#!/bin/bash
# Disabled by GreenUp Link installer
exit 0
EOF
    chmod +x "$INIT_SCRIPT"
    update-rc.d -f CommunicationArduinoRasp remove 2>/dev/null || true
    echo "  -> Neutralized $INIT_SCRIPT with 'exit 0'"
else
    echo "  -> Notice: $INIT_SCRIPT not found (skipping)"
fi

# 4. Neutralize emergency DeletAll.sh script
echo ""
echo "[3/5] Neutralizing emergency DeletAll.sh script..."
CANDIDATE_SCRIPTS=(
    "/home/pi/Desktop/DeletAll.sh"
    "/home/pi/Desktop/deleteAll.sh"
    "/home/pi/DeletAll.sh"
    "/home/pi/deleteAll.sh"
)
FOUND_DELETE_SCRIPT=false
for SCRIPT in "${CANDIDATE_SCRIPTS[@]}"; do
    if [ -f "$SCRIPT" ]; then
        FOUND_DELETE_SCRIPT=true
        if [ ! -f "$SCRIPT.old" ]; then
            cp -p "$SCRIPT" "$SCRIPT.old"
            echo "  -> Backup created: $SCRIPT.old"
        fi
        cat << 'EOF' > "$SCRIPT"
#!/bin/bash
# Neutralized by GreenUp Link installer
exit 0
EOF
        chmod -x "$SCRIPT"
        echo "  -> Neutralized: $SCRIPT (chmod -x applied)"
    fi
done
if [ "$FOUND_DELETE_SCRIPT" = false ]; then
    echo "  -> Notice: No DeletAll.sh script found (skipping)"
fi

# 5. Install Green'Up Link binary
echo ""
echo "[4/5] Installing Green'Up Link binary to $TARGET_DIR..."
mkdir -p "$TARGET_DIR"

if [ -f "$SCRIPT_DIR/greenup-link" ] && [ "$(realpath "$SCRIPT_DIR/greenup-link" 2>/dev/null || echo "$SCRIPT_DIR/greenup-link")" != "$TARGET_BIN" ]; then
    mv -f "$SCRIPT_DIR/greenup-link" "$TARGET_BIN"
    echo "  -> Moved $SCRIPT_DIR/greenup-link to $TARGET_BIN"
elif [ -f "./greenup-link" ] && [ "$(realpath ./greenup-link 2>/dev/null || echo "./greenup-link")" != "$TARGET_BIN" ]; then
    mv -f "./greenup-link" "$TARGET_BIN"
    echo "  -> Moved ./greenup-link to $TARGET_BIN"
elif [ -f "$TARGET_BIN" ]; then
    echo "  -> Binary already present at $TARGET_BIN"
else
    echo "❌ Error: 'greenup-link' binary not found!"
    echo "   Please place the 'greenup-link' executable in the same directory as this script."
    exit 1
fi

chmod +x "$TARGET_BIN"
chown -R pi:pi "$TARGET_DIR" 2>/dev/null || true

# 6. Configure and start Systemd service
echo ""
echo "[5/5] Configuring and starting Systemd service..."
SERVICE_FILE="/etc/systemd/system/greenup-link.service"

cat << 'EOF' > "$SERVICE_FILE"
[Unit]
Description=GreenUp Link Service
After=network.target

[Service]
Type=simple
WorkingDirectory=/home/pi/greenup
ExecStart=/home/pi/greenup/greenup-link
StandardOutput=inherit
StandardError=inherit
Restart=always
RestartSec=5s
User=root

[Install]
WantedBy=multi-user.target
EOF

echo "  -> Created $SERVICE_FILE"

systemctl daemon-reload
systemctl enable greenup-link.service
systemctl restart greenup-link.service

echo ""
echo "================================================="
sleep 1
if systemctl is-active --quiet greenup-link.service; then
    IP_ADDR=$(hostname -I 2>/dev/null | awk '{print $1}' || echo "your-pi-ip")
    echo "🎉 Success: Green'Up Link is active and running!"
    echo "   Web Interface: http://${IP_ADDR}:8080"
    echo "   Service Status: sudo systemctl status greenup-link"
    echo "   Live Logs:      sudo journalctl -u greenup-link -f"
else
    echo "⚠️ Warning: Service was configured and started, but is not reporting active yet."
    echo "   Check details with: sudo systemctl status greenup-link"
fi
echo "================================================="
