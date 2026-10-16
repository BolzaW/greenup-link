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
elif [ -f "$SCRIPT_DIR/greenup-link-armv7-legacy" ]; then
    mv -f "$SCRIPT_DIR/greenup-link-armv7-legacy" "$TARGET_BIN"
    echo "  -> Moved $SCRIPT_DIR/greenup-link-armv7-legacy to $TARGET_BIN"
elif [ -f "./greenup-link-armv7-legacy" ]; then
    mv -f "./greenup-link-armv7-legacy" "$TARGET_BIN"
    echo "  -> Moved ./greenup-link-armv7-legacy to $TARGET_BIN"
elif [ -f "$SCRIPT_DIR/greenup-link-aarch64" ]; then
    mv -f "$SCRIPT_DIR/greenup-link-aarch64" "$TARGET_BIN"
    echo "  -> Moved $SCRIPT_DIR/greenup-link-aarch64 to $TARGET_BIN"
elif [ -f "./greenup-link-aarch64" ]; then
    mv -f "./greenup-link-aarch64" "$TARGET_BIN"
    echo "  -> Moved ./greenup-link-aarch64 to $TARGET_BIN"
elif [ -f "$TARGET_BIN" ] && [ "$1" != "--update" ] && [ "$1" != "-u" ]; then
    echo "  -> Using existing binary at $TARGET_BIN (pass --update to fetch latest from GitHub)"
else
    echo "  -> 'greenup-link' binary not found locally."
    echo "  -> Detecting architecture and downloading latest release from GitHub..."

    RAW_ARCH=$(uname -m)
    echo "  -> Detected system architecture: $RAW_ARCH"
    case "$RAW_ARCH" in
        aarch64|arm64)
            TARGET_ARCH="aarch64"
            ;;
        armv7*|armv6*|armhf)
            TARGET_ARCH="armv7"
            ;;
        *)
            TARGET_ARCH="$RAW_ARCH"
            ;;
    esac
    echo "  -> Target binary architecture: $TARGET_ARCH"

    if ! command -v curl >/dev/null 2>&1 && ! command -v wget >/dev/null 2>&1; then
        echo "❌ Error: Neither 'curl' nor 'wget' is available to download the binary."
        echo "   Please install curl with: sudo apt-get install -y curl"
        exit 1
    fi

    echo "  -> Querying GitHub Releases API (BolzaW/greenup-link)..."
    API_URL="https://api.github.com/repos/BolzaW/greenup-link/releases/latest"
    if command -v curl >/dev/null 2>&1; then
        RELEASE_JSON=$(curl -sSL "$API_URL" || true)
    else
        RELEASE_JSON=$(wget -qO- "$API_URL" || true)
    fi

    if [ -z "$RELEASE_JSON" ]; then
        echo "❌ Error: Failed to retrieve release information from GitHub."
        echo "   Check network connectivity or provide the binary locally."
        exit 1
    fi

    URLS=$(echo "$RELEASE_JSON" | grep '"browser_download_url":' | sed -E 's/.*"browser_download_url": "([^"]+)".*/\1/' || true)

    DOWNLOAD_URL=""
    if [ "$TARGET_ARCH" = "aarch64" ]; then
        DOWNLOAD_URL=$(echo "$URLS" | grep -Ei 'aarch64|arm64' | head -n 1 || true)
    elif [ "$TARGET_ARCH" = "armv7" ]; then
        DOWNLOAD_URL=$(echo "$URLS" | grep -Ei 'armv7|legacy|armhf' | head -n 1 || true)
        if [ -z "$DOWNLOAD_URL" ]; then
            DOWNLOAD_URL=$(echo "$URLS" | grep -E '/greenup-link$' | head -n 1 || true)
        fi
    else
        DOWNLOAD_URL=$(echo "$URLS" | grep -Ei "$TARGET_ARCH" | head -n 1 || true)
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        NUM_URLS=$(echo "$URLS" | grep -c '^http' || true)
        if [ "$NUM_URLS" -eq 1 ]; then
            DOWNLOAD_URL=$(echo "$URLS" | head -n 1)
        fi
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        echo "❌ Error: Could not find a suitable asset for architecture '$TARGET_ARCH' in latest release."
        echo "   Available release assets:"
        echo "$URLS"
        exit 1
    fi

    echo "  -> Downloading: $DOWNLOAD_URL"
    TMP_FILE=$(mktemp /tmp/greenup-pkg-XXXXXX)

    if command -v curl >/dev/null 2>&1; then
        curl -sSL "$DOWNLOAD_URL" -o "$TMP_FILE"
    else
        wget -qO "$TMP_FILE" "$DOWNLOAD_URL"
    fi

    if [ ! -s "$TMP_FILE" ]; then
        echo "❌ Error: Download failed or resulted in an empty file."
        rm -f "$TMP_FILE"
        exit 1
    fi

    if [[ "$DOWNLOAD_URL" == *.zip ]]; then
        echo "  -> Extracting binary from ZIP archive..."
        EXTRACTED=false
        if command -v unzip >/dev/null 2>&1; then
            unzip -j -o -q "$TMP_FILE" "*greenup-link" -d "$TARGET_DIR" 2>/dev/null && EXTRACTED=true || true
        fi

        if [ "$EXTRACTED" = false ] && command -v python3 >/dev/null 2>&1; then
            python3 -c "
import zipfile, sys
with zipfile.ZipFile('$TMP_FILE', 'r') as z:
    for name in z.namelist():
        if name == 'greenup-link' or name.endswith('/greenup-link'):
            with open('$TARGET_BIN', 'wb') as f:
                f.write(z.read(name))
            sys.exit(0)
sys.exit(1)
" 2>/dev/null && EXTRACTED=true || true
        fi

        if [ "$EXTRACTED" = false ] && command -v python >/dev/null 2>&1; then
            python -c "
import zipfile, sys
with zipfile.ZipFile('$TMP_FILE', 'r') as z:
    for name in z.namelist():
        if name == 'greenup-link' or name.endswith('/greenup-link'):
            with open('$TARGET_BIN', 'wb') as f:
                f.write(z.read(name))
            sys.exit(0)
sys.exit(1)
" 2>/dev/null && EXTRACTED=true || true
        fi

        rm -f "$TMP_FILE"

        if [ "$EXTRACTED" = false ] || [ ! -f "$TARGET_BIN" ]; then
            echo "❌ Error: Failed to extract 'greenup-link' from downloaded zip archive."
            echo "   Please install 'unzip' using: sudo apt-get install -y unzip"
            exit 1
        fi
    else
        mv -f "$TMP_FILE" "$TARGET_BIN"
    fi

    echo "  -> Installed binary to $TARGET_BIN"
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
