// Configuration & System Panel module
// Handles runtime settings: Bluetooth (BLE) toggle, TIC detection test, Serial Init/Reset, Client API Token, and specs display.

function updateClientTokenBadge() {
    const badge = document.getElementById('cfgTokenStatusBadge');
    if (!badge) return;

    const token = typeof getApiToken === 'function' ? getApiToken() : '';

    if (window.isAuthRequired) {
        if (window.isAuthenticated) {
            badge.className = 'text-[10px] font-mono px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/30';
            badge.innerText = (typeof t === 'function' ? t('system.token_status_auth') : 'Authentifié') || 'Authentifié';
        } else if (token) {
            badge.className = 'text-[10px] font-mono px-2 py-0.5 rounded bg-rose-500/10 text-rose-400 border border-rose-500/30';
            badge.innerText = (typeof t === 'function' ? t('system.token_status_unauth') : 'Non authentifié') || 'Non authentifié';
        } else {
            badge.className = 'text-[10px] font-mono px-2 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/30';
            badge.innerText = (typeof t === 'function' ? t('system.token_status_missing') : 'Token manquant') || 'Token manquant';
        }
    } else {
        badge.className = 'text-[10px] font-mono px-2 py-0.5 rounded bg-slate-800 text-slate-400 border border-slate-700';
        badge.innerText = (typeof t === 'function' ? t('system.token_status_no_auth') : 'Accès libre (pas d'auth)') || 'Accès libre (pas d'auth)';
    }
}

function toggleTokenVisibility() {
    const input = document.getElementById('cfgClientToken');
    const iconEye = document.getElementById('iconEye');
    const iconEyeSlash = document.getElementById('iconEyeSlash');
    if (!input) return;

    if (input.type === 'password') {
        input.type = 'text';
        if (iconEye) iconEye.classList.add('hidden');
        if (iconEyeSlash) iconEyeSlash.classList.remove('hidden');
    } else {
        input.type = 'password';
        if (iconEye) iconEye.classList.remove('hidden');
        if (iconEyeSlash) iconEyeSlash.classList.add('hidden');
    }
}

async function saveClientToken() {
    const input = document.getElementById('cfgClientToken');
    const feedback = document.getElementById('cfgTokenFeedback');
    const candidate = input ? input.value.trim() : '';

    if (typeof setApiToken === 'function') {
        setApiToken(candidate);
    }

    try {
        const headers = candidate ? { 'Authorization': `Bearer ${candidate}` } : {};
        const res = await fetch('/api/auth/status', { headers });
        const status = await res.json();

        window.isAuthRequired = !!status.auth_required;
        window.isAuthenticated = !!status.authenticated;
        if (typeof updateLogoutButton === 'function') updateLogoutButton();
        updateClientTokenBadge();

        if (status.auth_required && !status.authenticated) {
            if (feedback) {
                feedback.innerText = (typeof t === 'function' ? t('system.token_invalid') : 'Clé API refusée par le serveur') || 'Clé API refusée par le serveur';
                feedback.className = 'text-[11px] font-medium text-rose-400 min-h-[16px] mt-1 pl-1';
            }
        } else if (status.auth_required && status.authenticated) {
            if (feedback) {
                feedback.innerText = (typeof t === 'function' ? t('system.token_saved_valid') : 'Clé API enregistrée & validée') || 'Clé API enregistrée & validée';
                feedback.className = 'text-[11px] font-medium text-emerald-400 min-h-[16px] mt-1 pl-1';
            }
            if (typeof hideAuthModal === 'function') hideAuthModal();
            if (typeof onAuthenticatedStart === 'function') {
                await onAuthenticatedStart();
            }
            if (typeof loadConfigData === 'function') {
                await loadConfigData();
            }
        } else {
            if (feedback) {
                feedback.innerText = (typeof t === 'function' ? t('system.token_saved_local') : 'Clé enregistrée localement') || 'Clé enregistrée localement';
                feedback.className = 'text-[11px] font-medium text-sky-400 min-h-[16px] mt-1 pl-1';
            }
        }
    } catch (err) {
        console.error('Save token verification error:', err);
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.token_saved_local') : 'Clé enregistrée localement') || 'Clé enregistrée localement';
            feedback.className = 'text-[11px] font-medium text-sky-400 min-h-[16px] mt-1 pl-1';
        }
        updateClientTokenBadge();
    } finally {
        if (feedback) {
            setTimeout(() => {
                feedback.innerText = '';
            }, 4000);
        }
    }
}

async function clearClientToken() {
    const input = document.getElementById('cfgClientToken');
    const feedback = document.getElementById('cfgTokenFeedback');
    if (input) input.value = '';

    if (typeof setApiToken === 'function') {
        setApiToken('');
    }
    if (typeof stopTelemetryPolling === 'function') {
        stopTelemetryPolling();
    }

    try {
        const res = await fetch('/api/auth/status');
        const status = await res.json();
        window.isAuthRequired = !!status.auth_required;
        window.isAuthenticated = !!status.authenticated;
        if (typeof updateLogoutButton === 'function') updateLogoutButton();
    } catch (_) {}

    updateClientTokenBadge();

    if (feedback) {
        feedback.innerText = (typeof t === 'function' ? t('system.token_cleared') : 'Clé effacée du navigateur') || 'Clé effacée du navigateur';
        feedback.className = 'text-[11px] font-medium text-amber-400 min-h-[16px] mt-1 pl-1';
        setTimeout(() => { feedback.innerText = ''; }, 3000);
    }
}

async function loadConfigData() {
    if (!window.isAuthResolved || (window.isAuthRequired && !window.isAuthenticated)) return;

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

        // Populate client API token input & status badge
        const tokenInput = document.getElementById('cfgClientToken');
        if (tokenInput && typeof getApiToken === 'function') {
            tokenInput.value = getApiToken();
        }
        updateClientTokenBadge();

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

    const btnToggleEye = document.getElementById('btnToggleTokenVisibility');
    if (btnToggleEye) btnToggleEye.addEventListener('click', toggleTokenVisibility);

    const btnSaveToken = document.getElementById('btnSaveClientToken');
    if (btnSaveToken) btnSaveToken.addEventListener('click', saveClientToken);

    const btnClearToken = document.getElementById('btnClearClientToken');
    if (btnClearToken) btnClearToken.addEventListener('click', clearClientToken);

    const tokenInput = document.getElementById('cfgClientToken');
    if (tokenInput) {
        tokenInput.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                e.preventDefault();
                saveClientToken();
            }
        });
    }
}
