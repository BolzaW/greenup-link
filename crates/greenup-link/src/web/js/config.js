// Configuration & Tools Panel module
// Handles runtime settings: Bluetooth (BLE) toggle, TIC detection test, and hardware/network display.

async function loadConfigData() {
    if (window.isAuthRequired && !window.isAuthenticated) return;

    try {
        // Sync hardware / BLE state from /api/info
        if (typeof fetchInfo === 'function') {
            await fetchInfo();
        }

        // Display current HTTP interface from browser location
        const bindElem = document.getElementById('cfgHttpBind');
        if (bindElem) {
            bindElem.innerText = window.location.host || '0.0.0.0:8080';
        }

        // Display serial port from telemetry if available
        const portElem = document.getElementById('cfgSerialPort');
        if (portElem && window.lastTelemetry && window.lastTelemetry.serial && window.lastTelemetry.serial.port) {
            portElem.innerText = window.lastTelemetry.serial.port;
        }
    } catch (e) {
        console.warn('Failed to refresh config panel info:', e);
    }
}

async function refreshTicFromConfig() {
    const feedback = document.getElementById('cfgTicFeedback');

    if (lastReportedState !== 'A') {
        if (feedback) {
            feedback.innerText = t('alert.tic_refresh_state_a') || "La borne doit être à l'état A (Libre) pour relancer la détection TIC.";
            feedback.className = 'text-[11px] font-medium text-rose-400 mt-1';
            setTimeout(() => { feedback.innerText = ''; }, 4000);
        }
        return;
    }

    try {
        if (feedback) {
            feedback.innerText = t('config.tic_refreshing') || "Détection TIC lancée sur l'ATmega...";
            feedback.className = 'text-[11px] font-medium text-sky-400 mt-1';
        }
        await apiFetch('/api/tic/refresh', { method: 'POST' });
        setTimeout(() => {
            if (feedback) feedback.innerText = '';
        }, 4000);
    } catch (err) {
        console.error('TIC refresh error:', err);
        if (feedback) {
            feedback.innerText = 'Erreur : ' + err;
            feedback.className = 'text-[11px] font-medium text-rose-400 mt-1';
        }
    }
}

function initConfig() {
    const btnTic = document.getElementById('btnRefreshTicConfig');
    if (btnTic) btnTic.addEventListener('click', refreshTicFromConfig);
}
