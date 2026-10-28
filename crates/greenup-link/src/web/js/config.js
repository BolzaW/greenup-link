// Configuration & System Panel module
// Handles runtime settings: Bluetooth (BLE) toggle, TIC detection test, Serial Init/Reset, and specs display.

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

        // Display app version if available
        const appVerElem = document.getElementById('cfgAppVersion');
        if (appVerElem && window.linkVersion) {
            appVerElem.innerText = `v${window.linkVersion}`;
        }

        // Display serial port and baudrate from telemetry if available
        if (window.lastTelemetry && window.lastTelemetry.serial) {
            const portElem = document.getElementById('cfgSerialPort');
            if (portElem && window.lastTelemetry.serial.port) {
                portElem.innerText = window.lastTelemetry.serial.port;
            }
            const baudElem = document.getElementById('cfgBaudRate');
            if (baudElem && window.lastTelemetry.serial.baud_rate) {
                baudElem.innerText = `${window.lastTelemetry.serial.baud_rate} bauds`;
            }
        }
    } catch (e) {
        console.warn('Failed to refresh config panel info:', e);
    }
}

async function refreshTicFromConfig() {
    const feedback = document.getElementById('cfgTicFeedback');
    const btn = document.getElementById('btnRefreshTicConfig');
    const icon = document.getElementById('iconRefreshTic');

    if (lastReportedState !== 'A') {
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('alert.tic_refresh_state_a') : "La borne doit être à l'état A (Libre) pour lancer une détection TIC.") || "La borne doit être à l'état A (Libre) pour lancer une détection TIC.";
            feedback.className = 'text-[11px] font-medium text-rose-400 mt-1';
            setTimeout(() => { feedback.innerText = ''; }, 4000);
        }
        return;
    }

    if (btn) btn.disabled = true;
    if (icon) icon.classList.add('animate-spin');

    try {
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.tic_refreshing') : "Détection TIC lancée sur l'ATmega...") || "Détection TIC lancée sur l'ATmega...";
            feedback.className = 'text-[11px] font-medium text-sky-400 mt-1 animate-pulse';
        }
        const res = await apiFetch('/api/tic/refresh', { method: 'POST' });
        if (!res.ok) {
            const errData = await res.json().catch(() => ({}));
            throw new Error(errData.error || ('HTTP ' + res.status));
        }
        setTimeout(() => {
            if (feedback) feedback.innerText = '';
        }, 4000);
    } catch (err) {
        console.error('TIC refresh error:', err);
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('alert.error') : 'Erreur: ') + (err.message || err);
            feedback.className = 'text-[11px] font-medium text-rose-400 mt-1';
        }
    } finally {
        if (icon) icon.classList.remove('animate-spin');
        if (btn) btn.disabled = false;
    }
}

async function handleSerialInit() {
    const btn = document.getElementById('btnSerialInit');
    const icon = document.getElementById('iconSerialInit');
    const feedback = document.getElementById('serialActionFeedback');

    if (btn) btn.disabled = true;
    if (icon) icon.classList.add('animate-spin');
    if (feedback) {
        feedback.innerText = (typeof t === 'function' ? t('system.init_sent') : "Séquence d'initialisation envoyée...") || "Séquence d'initialisation envoyée...";
        feedback.className = 'text-[11px] font-medium text-sky-400 text-center min-h-[16px] mt-1.5 animate-pulse';
    }

    try {
        const res = await apiFetch('/api/init', { method: 'POST' });
        if (!res.ok) {
            const errData = await res.json().catch(() => ({}));
            throw new Error(errData.error || errData.message || ('HTTP ' + res.status));
        }
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.init_success') : "Initialisation terminée avec succès") || "Initialisation terminée avec succès";
            feedback.className = 'text-[11px] font-medium text-emerald-400 text-center min-h-[16px] mt-1.5';
        }
    } catch (err) {
        console.error('Serial init error:', err);
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('alert.error') : 'Erreur: ') + (err.message || err);
            feedback.className = 'text-[11px] font-medium text-rose-400 text-center min-h-[16px] mt-1.5';
        }
    } finally {
        if (icon) icon.classList.remove('animate-spin');
        if (btn) btn.disabled = false;
        if (feedback) {
            setTimeout(() => {
                feedback.innerText = '';
                feedback.className = 'text-[11px] font-medium text-center min-h-[16px] mt-1.5';
            }, 4000);
        }
    }
}

async function handleSerialReset() {
    const btn = document.getElementById('btnSerialReset');
    const icon = document.getElementById('iconSerialReset');
    const feedback = document.getElementById('serialActionFeedback');

    if (btn) btn.disabled = true;
    if (icon) icon.classList.add('animate-spin');
    if (feedback) {
        feedback.innerText = (typeof t === 'function' ? t('system.reset_sent') : "Commande de redémarrage envoyée...") || "Commande de redémarrage envoyée...";
        feedback.className = 'text-[11px] font-medium text-amber-400 text-center min-h-[16px] mt-1.5 animate-pulse';
    }

    try {
        const res = await apiFetch('/api/reset', { method: 'POST' });
        if (!res.ok) {
            const errData = await res.json().catch(() => ({}));
            throw new Error(errData.error || errData.message || ('HTTP ' + res.status));
        }
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.reset_success') : "Redémarrage demandé avec succès") || "Redémarrage demandé avec succès";
            feedback.className = 'text-[11px] font-medium text-emerald-400 text-center min-h-[16px] mt-1.5';
        }
    } catch (err) {
        console.error('Serial reset error:', err);
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('alert.error') : 'Erreur: ') + (err.message || err);
            feedback.className = 'text-[11px] font-medium text-rose-400 text-center min-h-[16px] mt-1.5';
        }
    } finally {
        if (icon) icon.classList.remove('animate-spin');
        if (btn) btn.disabled = false;
        if (feedback) {
            setTimeout(() => {
                feedback.innerText = '';
                feedback.className = 'text-[11px] font-medium text-center min-h-[16px] mt-1.5';
            }, 4000);
        }
    }
}

function initConfig() {
    const btnTic = document.getElementById('btnRefreshTicConfig');
    if (btnTic) btnTic.addEventListener('click', refreshTicFromConfig);

    const btnInit = document.getElementById('btnSerialInit');
    if (btnInit) btnInit.addEventListener('click', handleSerialInit);

    const btnReset = document.getElementById('btnSerialReset');
    if (btnReset) btnReset.addEventListener('click', handleSerialReset);
}
