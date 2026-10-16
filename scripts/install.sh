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

# 2. Environment check: Detect official Legrand software stack
echo ""
echo "[1/4] Checking system environment..."
IS_LEGRAND_STACK=false
if [ -f "/etc/init.d/CommunicationArduinoRasp" ] || [ -f "/etc/init.d/CommunicationArduinoRasp.old" ] || [ -f "/home/pi/Desktop/DeletAll.sh" ] || [ -d "/var/lib/tomcat9" ]; then
    IS_LEGRAND_STACK=true
fi

if [ "$IS_LEGRAND_STACK" = true ]; then
    echo "  -> Official Legrand software stack detected."

    # A. Stop and disable legacy Legrand services (Tomcat9 & Java daemon)
    echo "  -> Stopping legacy Legrand services (Tomcat9 & Java daemon)..."
    systemctl stop tomcat9 2>/dev/null || true
    systemctl disable tomcat9 2>/dev/null || true
    pkill -9 -f java 2>/dev/null || true
    pkill -9 -f tomcat 2>/dev/null || true
    echo "  -> Legacy Java and Tomcat processes stopped."

    # B. Disable Legrand init.d startup script
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
    fi

    # C. Neutralize emergency DeletAll.sh script
    DELETE_SCRIPT="/home/pi/Desktop/DeletAll.sh"
    if [ -f "$DELETE_SCRIPT" ]; then
        if [ ! -f "$DELETE_SCRIPT.old" ]; then
            cp -p "$DELETE_SCRIPT" "$DELETE_SCRIPT.old"
            echo "  -> Backup created: $DELETE_SCRIPT.old"
        fi
        cat << 'EOF' > "$DELETE_SCRIPT"
#!/bin/bash
# Neutralized by GreenUp Link installer
exit 0
EOF
        chmod -x "$DELETE_SCRIPT"
        echo "  -> Neutralized: $DELETE_SCRIPT (chmod -x applied)"
    fi
else
    echo "  -> Standard/clean OS detected (no Legrand stack found, skipping legacy neutralization)."
fi

# 3. Install Green'Up Link binary
echo ""
echo "[2/4] Installing Green'Up Link binary to $TARGET_DIR..."
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
elif [ -f "$TARGET_BIN" ] && { [ "$1" = "--reconfigure" ] || [ "$1" = "--offline" ] || [ "$1" = "--no-download" ]; }; then
    echo "  -> Keeping existing binary at $TARGET_BIN (--offline/--reconfigure mode)"
else
    if [ -f "$TARGET_BIN" ]; then
        echo "  -> Existing installation detected at $TARGET_BIN."
        echo "  -> Fetching and upgrading to the latest release from GitHub..."
    else
        echo "  -> 'greenup-link' binary not found locally."
        echo "  -> Detecting architecture and downloading latest release from GitHub..."
    fi

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

    # Extract asset URLs from release JSON
    URLS=$(echo "$RELEASE_JSON" | grep '"browser_download_url":' | sed -E 's/.*"browser_download_url": "([^"]+)".*/\1/' || true)

    # Filter out archive files (.zip, .tar, .gz) and checksums (.txt) to target standalone binaries directly
    BIN_URLS=$(echo "$URLS" | grep -vE '\.(zip|tar|gz|tgz|txt|md)$' || true)

    DOWNLOAD_URL=""
    if [ "$TARGET_ARCH" = "aarch64" ]; then
        DOWNLOAD_URL=$(echo "$BIN_URLS" | grep -Ei 'greenup-link-aarch64|greenup-link-arm64' | head -n 1 || true)
    elif [ "$TARGET_ARCH" = "armv7" ]; then
        DOWNLOAD_URL=$(echo "$BIN_URLS" | grep -Ei 'greenup-link-armv7-legacy|greenup-link-armv7|greenup-link-legacy' | head -n 1 || true)
    fi

    # Fallback to any matching binary URL if specific pattern not found
    if [ -z "$DOWNLOAD_URL" ]; then
        DOWNLOAD_URL=$(echo "$BIN_URLS" | grep -Ei "$TARGET_ARCH" | head -n 1 || true)
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        echo "❌ Error: Could not find standalone binary for architecture '$TARGET_ARCH' in latest release."
        echo "   Available release assets:"
        echo "$URLS"
        exit 1
    fi

    echo "  -> Downloading binary: $DOWNLOAD_URL"
    TMP_FILE=$(mktemp /tmp/greenup-bin-XXXXXX)

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

    mv -f "$TMP_FILE" "$TARGET_BIN"
    echo "  -> Installed binary to $TARGET_BIN"
fi

chmod +x "$TARGET_BIN"

# 4. Install uninstaller into target directory
echo ""
echo "[3/4] Installing rollback/uninstall script to $UNINSTALL_TARGET..."
UNINSTALL_TARGET="$TARGET_DIR/uninstall.sh"
if [ -f "$SCRIPT_DIR/uninstall.sh" ]; then
    cp -p "$SCRIPT_DIR/uninstall.sh" "$UNINSTALL_TARGET"
    chmod +x "$UNINSTALL_TARGET"
    echo "  -> Installed uninstaller to $UNINSTALL_TARGET"
elif [ -f "./uninstall.sh" ]; then
    cp -p "./uninstall.sh" "$UNINSTALL_TARGET"
    chmod +x "$UNINSTALL_TARGET"
    echo "  -> Installed uninstaller to $UNINSTALL_TARGET"
else
    UNINSTALL_URL="https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/uninstall.sh"
    if command -v curl >/dev/null 2>&1; then
        curl -sSL "$UNINSTALL_URL" -o "$UNINSTALL_TARGET" 2>/dev/null && chmod +x "$UNINSTALL_TARGET" || true
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$UNINSTALL_TARGET" "$UNINSTALL_URL" 2>/dev/null && chmod +x "$UNINSTALL_TARGET" || true
    fi
    if [ -f "$UNINSTALL_TARGET" ]; then
        echo "  -> Installed uninstaller to $UNINSTALL_TARGET"
    fi
fi

chown -R pi:pi "$TARGET_DIR" 2>/dev/null || true

# 5. Configure and start Systemd service
echo ""
echo "[4/4] Configuring and starting Systemd service..."
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
    echo "   Web Interface:  http://${IP_ADDR}:8080"
    echo "   Service Status: sudo systemctl status greenup-link"
    echo "   Live Logs:       sudo journalctl -u greenup-link -f"
    echo "   Rollback/Remove: sudo /home/pi/greenup/uninstall.sh"
else
    echo "⚠️ Warning: Service was configured and started, but is not reporting active yet."
    echo "   Check details with: sudo systemctl status greenup-link"
fi
echo "================================================="
