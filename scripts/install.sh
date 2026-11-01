#!/bin/bash
set -e

# ==============================================================================
# Green'Up Link - Automated Installation Script for Raspberry Pi
# ==============================================================================

echo "================================================="
echo "             Green'Up Link - Installer           "
echo "================================================="

# 1. Require root privileges
if [ "$EUID" -ne 0 ]; then
    echo "❌ Error: This script must be run as root."
    echo "   Please re-run with: sudo $0"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="/opt/greenup-link"
TARGET_BIN="$TARGET_DIR/greenup-link"

# Create dedicated unprivileged system user 'greenup' if it does not exist
echo "  -> Setting up dedicated unprivileged system user 'greenup'..."
if ! id -u greenup >/dev/null 2>&1; then
    useradd --system --no-create-home --home-dir "$TARGET_DIR" --shell /usr/sbin/nologin greenup
    echo "  -> Created system user 'greenup'"
else
    echo "  -> System user 'greenup' already exists"
fi

# Ensure greenup user belongs to 'dialout' group (required for /dev/ttyUSB* serial access)
if getent group dialout >/dev/null 2>&1; then
    usermod -a -G dialout greenup 2>/dev/null || true
    echo "  -> Added 'greenup' user to group 'dialout' (serial port access)"
fi

# Also add to 'gpio' group if present on Raspberry Pi OS
if getent group gpio >/dev/null 2>&1; then
    usermod -a -G gpio greenup 2>/dev/null || true
fi

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

# Helper functions for SHA256 checksum verification
verify_sha256() {
    local target_file="$1"
    local filename="$2"
    local checksums_file="$3"

    if ! command -v sha256sum >/dev/null 2>&1; then
        echo "  -> Warning: 'sha256sum' utility not found, skipping checksum verification."
        return 0
    fi

    if [ ! -f "$checksums_file" ]; then
        return 0
    fi

    local expected_hash
    expected_hash=$(awk -v fn="$filename" '$2 == fn || $2 == "*"fn {print $1}' "$checksums_file" | head -n 1)

    if [ -z "$expected_hash" ]; then
        echo "  -> Note: No SHA256 checksum entry found for '$filename' in checksum file, skipping verification."
        return 0
    fi

    echo "  -> Verifying SHA256 integrity for $filename..."
    local actual_hash
    actual_hash=$(sha256sum "$target_file" | awk '{print $1}')

    if [ "$expected_hash" != "$actual_hash" ]; then
        echo "❌ Error: SHA256 checksum mismatch for $filename!"
        echo "   Expected: $expected_hash"
        echo "   Actual:   $actual_hash"
        return 1
    fi

    echo "  -> Verified SHA256 checksum: $actual_hash (OK)"
    return 0
}

verify_local_sha256() {
    local target_file="$1"
    local filename="$2"
    local checksums_file=""

    if [ -f "$SCRIPT_DIR/SHA256SUMS.txt" ]; then
        checksums_file="$SCRIPT_DIR/SHA256SUMS.txt"
    elif [ -f "./SHA256SUMS.txt" ]; then
        checksums_file="./SHA256SUMS.txt"
    fi

    if [ -n "$checksums_file" ]; then
        verify_sha256 "$target_file" "$filename" "$checksums_file"
    fi
}

# 3. Install Green'Up Link binary
echo ""
echo "[2/4] Installing Green'Up Link binary to $TARGET_DIR..."
mkdir -p "$TARGET_DIR"

# Stop active service if updating to prevent "Text file busy" error
if systemctl is-active --quiet greenup-link.service 2>/dev/null; then
    echo "  -> Stopping active greenup-link service before binary update..."
    systemctl stop greenup-link.service 2>/dev/null || true
fi

if [ -f "$SCRIPT_DIR/greenup-link" ] && [ "$(realpath "$SCRIPT_DIR/greenup-link" 2>/dev/null || echo "$SCRIPT_DIR/greenup-link")" != "$TARGET_BIN" ]; then
    verify_local_sha256 "$SCRIPT_DIR/greenup-link" "greenup-link" || exit 1
    mv -f "$SCRIPT_DIR/greenup-link" "$TARGET_BIN"
    echo "  -> Moved $SCRIPT_DIR/greenup-link to $TARGET_BIN"
elif [ -f "./greenup-link" ] && [ "$(realpath ./greenup-link 2>/dev/null || echo "./greenup-link")" != "$TARGET_BIN" ]; then
    verify_local_sha256 "./greenup-link" "greenup-link" || exit 1
    mv -f "./greenup-link" "$TARGET_BIN"
    echo "  -> Moved ./greenup-link to $TARGET_BIN"
elif [ -f "$SCRIPT_DIR/greenup-link-armv7-legacy" ]; then
    verify_local_sha256 "$SCRIPT_DIR/greenup-link-armv7-legacy" "greenup-link-armv7-legacy" || exit 1
    mv -f "$SCRIPT_DIR/greenup-link-armv7-legacy" "$TARGET_BIN"
    echo "  -> Moved $SCRIPT_DIR/greenup-link-armv7-legacy to $TARGET_BIN"
elif [ -f "./greenup-link-armv7-legacy" ]; then
    verify_local_sha256 "./greenup-link-armv7-legacy" "greenup-link-armv7-legacy" || exit 1
    mv -f "./greenup-link-armv7-legacy" "$TARGET_BIN"
    echo "  -> Moved ./greenup-link-armv7-legacy to $TARGET_BIN"
elif [ -f "$SCRIPT_DIR/greenup-link-aarch64" ]; then
    verify_local_sha256 "$SCRIPT_DIR/greenup-link-aarch64" "greenup-link-aarch64" || exit 1
    mv -f "$SCRIPT_DIR/greenup-link-aarch64" "$TARGET_BIN"
    echo "  -> Moved $SCRIPT_DIR/greenup-link-aarch64 to $TARGET_BIN"
elif [ -f "./greenup-link-aarch64" ]; then
    verify_local_sha256 "./greenup-link-aarch64" "greenup-link-aarch64" || exit 1
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

    BIN_FILENAME=$(basename "$DOWNLOAD_URL")
    echo "  -> Downloading binary: $BIN_FILENAME ($DOWNLOAD_URL)"
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

    # Download SHA256SUMS.txt if available in release assets
    CHECKSUMS_URL=$(echo "$URLS" | grep -Ei 'sha256sums\.txt' | head -n 1 || true)
    if [ -n "$CHECKSUMS_URL" ]; then
        TMP_CHECKSUMS=$(mktemp /tmp/greenup-sums-XXXXXX)
        if command -v curl >/dev/null 2>&1; then
            curl -sSL "$CHECKSUMS_URL" -o "$TMP_CHECKSUMS" 2>/dev/null || true
        else
            wget -qO "$TMP_CHECKSUMS" "$CHECKSUMS_URL" 2>/dev/null || true
        fi

        if [ -s "$TMP_CHECKSUMS" ]; then
            if ! verify_sha256 "$TMP_FILE" "$BIN_FILENAME" "$TMP_CHECKSUMS"; then
                rm -f "$TMP_FILE" "$TMP_CHECKSUMS"
                exit 1
            fi
        fi
        rm -f "$TMP_CHECKSUMS"
    else
        echo "  -> Warning: SHA256SUMS.txt not found in release assets, skipping verification."
    fi

    mv -f "$TMP_FILE" "$TARGET_BIN"
    echo "  -> Installed binary to $TARGET_BIN"
fi

chmod +x "$TARGET_BIN"

# 4. Install uninstaller into target directory
UNINSTALL_TARGET="$TARGET_DIR/uninstall.sh"
echo ""
echo "[3/4] Installing rollback/uninstall script to $UNINSTALL_TARGET..."
if [ -f "$SCRIPT_DIR/uninstall.sh" ] && [ "$(realpath "$SCRIPT_DIR/uninstall.sh" 2>/dev/null || echo "$SCRIPT_DIR/uninstall.sh")" != "$UNINSTALL_TARGET" ]; then
    cp -p "$SCRIPT_DIR/uninstall.sh" "$UNINSTALL_TARGET"
    chmod +x "$UNINSTALL_TARGET"
    echo "  -> Installed uninstaller to $UNINSTALL_TARGET"
elif [ -f "./uninstall.sh" ] && [ "$(realpath ./uninstall.sh 2>/dev/null || echo "./uninstall.sh")" != "$UNINSTALL_TARGET" ]; then
    cp -p "./uninstall.sh" "$UNINSTALL_TARGET"
    chmod +x "$UNINSTALL_TARGET"
    echo "  -> Installed uninstaller to $UNINSTALL_TARGET"
elif [ -f "$UNINSTALL_TARGET" ] && { [ "$1" = "--reconfigure" ] || [ "$1" = "--offline" ] || [ "$1" = "--no-download" ]; }; then
    chmod +x "$UNINSTALL_TARGET"
    echo "  -> Keeping existing uninstaller at $UNINSTALL_TARGET"
else
    UNINSTALL_URL="https://raw.githubusercontent.com/BolzaW/greenup-link/main/scripts/uninstall.sh"
    if command -v curl >/dev/null 2>&1; then
        curl -sSL "$UNINSTALL_URL" -o "$UNINSTALL_TARGET" 2>/dev/null && chmod +x "$UNINSTALL_TARGET" || true
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$UNINSTALL_TARGET" "$UNINSTALL_URL" 2>/dev/null && chmod +x "$UNINSTALL_TARGET" || true
    fi
    if [ -f "$UNINSTALL_TARGET" ]; then
        chmod +x "$UNINSTALL_TARGET"
        echo "  -> Installed uninstaller to $UNINSTALL_TARGET"
    fi
fi

CONFIG_FILE="$TARGET_DIR/greenup.toml"
if [ ! -f "$CONFIG_FILE" ]; then
    cat << 'EOF' > "$CONFIG_FILE"
# ==============================================================================
# Green'Up Link - Configuration File
# ==============================================================================

[http]
# Network interface and port to listen on.
# Default: "0.0.0.0:8080" (all network interfaces).
bind = "0.0.0.0:8080"

# Optional API token / password to protect the Web GUI and REST API.
# - Leave empty ("") to disable authentication (open access on trusted LAN).
# - Set a secret string to require authentication on all /api/* routes.
#   Compatible with:
#     * Bearer Token : Authorization: Bearer <api_token>  (EVCC, Home Assistant, Web GUI)
#     * API Key      : X-API-Key: <api_token>
#     * Basic Auth   : Authorization: Basic <base64(user:api_token)>  (curl -u :token)
api_token = ""
EOF
    chmod 640 "$CONFIG_FILE"
    echo "  -> Created default configuration file at $CONFIG_FILE"
else
    chmod 640 "$CONFIG_FILE" 2>/dev/null || true
    echo "  -> Keeping existing configuration file at $CONFIG_FILE"
fi

# Ensure proper ownership and permissions for the dedicated user
mkdir -p "$TARGET_DIR/logs"
chown -R greenup:greenup "$TARGET_DIR"
chmod 750 "$TARGET_DIR"
chmod 755 "$TARGET_BIN"
[ -f "$UNINSTALL_TARGET" ] && chmod 755 "$UNINSTALL_TARGET"

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
User=greenup
Group=greenup
WorkingDirectory=/opt/greenup-link
ExecStart=/opt/greenup-link/greenup-link
StandardOutput=inherit
StandardError=inherit
Restart=always
RestartSec=5s

# Security hardening
ProtectSystem=full
ProtectHome=true
NoNewPrivileges=true
PrivateTmp=true

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
    echo "   Web Interface:   http://${IP_ADDR}:8080"
    echo "   Directory:       /opt/greenup-link"
    echo "   Config & Auth:   /opt/greenup-link/greenup.toml"
    echo "   Logs:            /opt/greenup-link/logs/"
    echo "   Service User:    greenup (dialout group)"
    echo "   Service Status:  sudo systemctl status greenup-link"
    echo "   Live Logs:       sudo journalctl -u greenup-link -f"
    echo "   Rollback/Remove: sudo /opt/greenup-link/uninstall.sh"
else
    echo "⚠️ Warning: Service was configured and started, but is not reporting active yet."
    echo "   Check details with: sudo systemctl status greenup-link"
fi
echo "================================================="
