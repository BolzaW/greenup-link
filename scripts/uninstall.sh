#!/bin/bash
set -e

# ==============================================================================
# Green'Up Link - Rollback & Uninstaller
# ==============================================================================

echo "================================================="
echo "     Green'Up Link - Rollback & Uninstaller      "
echo "================================================="

# 1. Require root privileges
if [ "$EUID" -ne 0 ]; then
    echo "❌ Error: This script must be run as root."
    echo "   Please re-run with: sudo $0"
    exit 1
fi

TARGET_DIR="/opt/greenup-link"
SERVICE_FILE="/etc/systemd/system/greenup-link.service"

# Detect whether this system has an official Legrand software stack
IS_LEGRAND_STACK=false
if [ -f "/etc/init.d/CommunicationArduinoRasp.old" ] || [ -f "/etc/init.d/CommunicationArduinoRasp" ] || [ -d "/var/lib/tomcat9" ]; then
    IS_LEGRAND_STACK=true
fi
for del_cand in /home/*/Desktop/DeletAll.sh /home/*/Desktop/DeletAll.sh.old; do
    if [ -f "$del_cand" ]; then
        IS_LEGRAND_STACK=true
        break
    fi
done

# Parse CLI flags or ask user interactively
PURGE_FILES=false
INTERACTIVE=true

for arg in "$@"; do
    case "$arg" in
        --purge|--delete|-p|-d)
            PURGE_FILES=true
            INTERACTIVE=false
            ;;
        --keep|-k)
            PURGE_FILES=false
            INTERACTIVE=false
            ;;
        --help|-h)
            echo "Usage: sudo $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --keep, -k      Rollback and disable Green'Up Link, keeping files in $TARGET_DIR"
            echo "  --purge, -p     Rollback and completely delete all Green'Up Link files and logs"
            echo "  --help, -h      Display this help message"
            exit 0
            ;;
    esac
done

if [ "$INTERACTIVE" = true ]; then
    echo ""
    echo "Please choose an uninstallation option:"
    echo "  [1] Rollback and KEEP files in $TARGET_DIR (Disable Green'Up Link only)"
    echo "  [2] Rollback and PURGE all files and logs in $TARGET_DIR (Full removal)"
    echo ""
    
    CHOICE=""
    if [ -t 0 ]; then
        read -r -p "Enter choice [1 or 2, default: 1]: " CHOICE
    elif [ -r /dev/tty ]; then
        read -r -p "Enter choice [1 or 2, default: 1]: " CHOICE < /dev/tty
    else
        echo "Non-interactive shell detected; defaulting to Option [1] (Keep files)."
        CHOICE="1"
    fi

    case "$CHOICE" in
        2)
            PURGE_FILES=true
            ;;
        *)
            PURGE_FILES=false
            ;;
    esac
fi

# 1. Stop and disable Green'Up Link service
echo ""
echo "[1/3] Stopping and removing Green'Up Link systemd service..."
if [ -f "$SERVICE_FILE" ] || systemctl is-active --quiet greenup-link.service 2>/dev/null; then
    systemctl stop greenup-link.service 2>/dev/null || true
    systemctl disable greenup-link.service 2>/dev/null || true
    rm -f "$SERVICE_FILE"
    systemctl daemon-reload
    echo "  -> Service greenup-link stopped and systemd unit removed."
else
    echo "  -> Service greenup-link was not active or installed."
fi
pkill -9 -x greenup-link 2>/dev/null || true

# 2. Restore Legrand software stack if present
echo ""
if [ "$IS_LEGRAND_STACK" = true ]; then
    echo "[2/3] Restoring original Legrand software stack..."
    
    # Restore Legrand init.d startup script
    INIT_SCRIPT="/etc/init.d/CommunicationArduinoRasp"
    if [ -f "$INIT_SCRIPT.old" ]; then
        mv -f "$INIT_SCRIPT.old" "$INIT_SCRIPT"
        chmod +x "$INIT_SCRIPT"
        update-rc.d CommunicationArduinoRasp defaults 2>/dev/null || true
        echo "  -> Restored $INIT_SCRIPT from backup and re-registered service."
    elif [ -f "$INIT_SCRIPT" ]; then
        echo "  -> Notice: $INIT_SCRIPT.old backup not found; leaving existing $INIT_SCRIPT."
    fi

    # Restore emergency DeletAll.sh script across user desktops
    for del_old in /home/*/Desktop/DeletAll.sh.old; do
        if [ -f "$del_old" ]; then
            del_orig="${del_old%.old}"
            mv -f "$del_old" "$del_orig"
            chmod +x "$del_orig"
            echo "  -> Restored $del_orig from backup."
        fi
    done

    # Re-enable and restart Legrand services
    if systemctl list-unit-files tomcat9.service 2>/dev/null | grep -q tomcat9; then
        echo "  -> Re-enabling and starting Tomcat 9..."
        systemctl enable tomcat9 2>/dev/null || true
        systemctl start tomcat9 2>/dev/null || true
    fi

    if [ -x "$INIT_SCRIPT" ]; then
        echo "  -> Starting Legrand CommunicationArduinoRasp service..."
        "$INIT_SCRIPT" start 2>/dev/null || true
    fi
else
    echo "[2/3] Standard OS detected (no Legrand factory stack to restore, skipping)."
fi

# 3. Handle Green'Up Link files
echo ""
echo "[3/3] Handling Green'Up Link application files..."
if [ "$PURGE_FILES" = true ]; then
    echo "  -> Purging $TARGET_DIR (binaries, logs, configs)..."
    rm -rf "$TARGET_DIR"
    echo "  -> $TARGET_DIR successfully removed."
    if id -u greenup >/dev/null 2>&1; then
        echo "  -> Removing dedicated system user 'greenup'..."
        userdel greenup 2>/dev/null || true
    fi
    if getent group greenup >/dev/null 2>&1; then
        groupdel greenup 2>/dev/null || true
    fi
else
    echo "  -> Keeping $TARGET_DIR intact (disabled state)."
    echo "     You can manually delete it at any time with: sudo rm -rf $TARGET_DIR"
fi

echo ""
echo "================================================="
echo "🎉 Rollback complete!"
echo "   - Green'Up Link service has been disabled."
if [ "$PURGE_FILES" = true ]; then
    echo "   - Application files have been completely purged."
else
    echo "   - Application files are preserved in $TARGET_DIR."
fi
if [ "$IS_LEGRAND_STACK" = true ]; then
    echo "   - Original Legrand stack (Tomcat, Java, init.d) has been restored."
else
    echo "   - Standard OS: clean system, no Legrand stack needed restoration."
fi
echo "================================================="
