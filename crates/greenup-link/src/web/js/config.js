// Configuration & System Panel module
// Handles runtime settings: Bluetooth (BLE) toggle, TIC detection test, Serial Init/Reset, Client API Token, and specs display.

function updateClientTokenBadge() {
    const badge = document.getElementById('cfgTokenStatusBadge');
    if (!badge) return;

    const token = typeof getApiToken === 'function' ? getApiToken() : '';

    if (window.isAuthRequired) {
        if (window.isAuthenticated) {
            badge.className = 'sys-badge sys-badge-emerald';
            badge.innerText = (typeof t === 'function' ? t('system.token_status_auth') : 'Authentifié') || 'Authentifié';
        } else if (token) {
            badge.className = 'sys-badge sys-badge-rose';
            badge.innerText = (typeof t === 'function' ? t('system.token_status_unauth') : 'Non authentifié') || 'Non authentifié';
        } else {
            badge.className = 'sys-badge sys-badge-amber';
            badge.innerText = (typeof t === 'function' ? t('system.token_status_missing') : 'Token manquant') || 'Token manquant';
        }
    } else {
        badge.className = 'sys-badge sys-badge-muted';
        badge.innerText = (typeof t === 'function' ? t('system.token_status_no_auth') : "Accès libre (pas d'auth)") || "Accès libre (pas d'auth)";
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
                feedback.className = 'sys-feedback sys-feedback-error';
            }
        } else if (status.auth_required && status.authenticated) {
            if (feedback) {
                feedback.innerText = (typeof t === 'function' ? t('system.token_saved_valid') : 'Clé API enregistrée & validée') || 'Clé API enregistrée & validée';
                feedback.className = 'sys-feedback sys-feedback-ok';
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
                feedback.className = 'sys-feedback sys-feedback-info';
            }
        }
    } catch (err) {
        console.error('Save token verification error:', err);
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.token_saved_local') : 'Clé enregistrée localement') || 'Clé enregistrée localement';
            feedback.className = 'sys-feedback sys-feedback-info';
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
        feedback.className = 'sys-feedback sys-feedback-warn';
        setTimeout(() => { feedback.innerText = ''; }, 3000);
    }
}

async function loadConfigData() {
    if (!window.isAuthResolved || (window.isAuthRequired && !window.isAuthenticated)) return;

    try {
        // Sync hardware / BLE state from /api/station
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

        // Sync supervision mode from /api/mode
        try {
            const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
            const modeRes = await apiFetch(`/api/mode${sideQuery}`);
            if (modeRes.ok) {
                const modeData = await modeRes.json();
                updateSupervisionModeUi(modeData.mode, modeData.can_switch);
            }
        } catch (_) {}

    } catch (e) {
        console.warn('Failed to refresh config panel info:', e);
    }
}

async function refreshTicFromConfig() {
    const feedback = document.getElementById('cfgTicFeedback');
    const btn = document.getElementById('btnRefreshTicConfig');
    const icon = document.getElementById('iconRefreshTic');

    if (window.stationHasTic === false) {
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.tic_not_supported') : "Fonction TIC non disponible sur ce modèle de borne.");
            feedback.className = 'sys-feedback sys-feedback-warn';
            setTimeout(() => { feedback.innerText = ''; }, 4000);
        }
        return;
    }

    if (lastReportedState && lastReportedState !== 'A') {
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('alert.tic_refresh_state_a') : "La borne doit être à l'état A (Libre) pour lancer une détection TIC.") || "La borne doit être à l'état A (Libre) pour lancer une détection TIC.";
            feedback.className = 'sys-feedback sys-feedback-error';
            setTimeout(() => { feedback.innerText = ''; }, 4000);
        }
        return;
    }

    if (btn) btn.disabled = true;
    if (icon) icon.classList.add('animate-spin');

    try {
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.tic_refreshing') : "Détection TIC lancée sur l'ATmega...") || "Détection TIC lancée sur l'ATmega...";
            feedback.className = 'sys-feedback sys-feedback-info animate-pulse';
        }
        if (typeof updateTicSpeedBadge === 'function') {
            updateTicSpeedBadge('detecting');
        }
        const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
        const res = await apiFetch(`/api/tic/refresh${sideQuery}`, { method: 'POST' });
        if (!res.ok) {
            const errData = await res.json().catch(() => ({}));
            throw new Error(errData.error || ('HTTP ' + res.status));
        }
        if (typeof fetchTelemetry === 'function') {
            fetchTelemetry();
        }
        setTimeout(() => {
            if (feedback) feedback.innerText = '';
        }, 4000);
    } catch (err) {
        console.error('TIC refresh error:', err);
        if (typeof updateTicSpeedBadge === 'function') {
            updateTicSpeedBadge(lastReportedTicMode);
        }
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('alert.error') : 'Erreur: ') + (err.message || err);
            feedback.className = 'sys-feedback sys-feedback-error';
        }
    } finally {
        if (icon) icon.classList.remove('animate-spin');
        if (btn && window.stationHasTic !== false) btn.disabled = false;
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
        feedback.className = 'sys-feedback sys-feedback-info animate-pulse';
    }

    try {
        const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
        const res = await apiFetch(`/api/init${sideQuery}`, { method: 'POST' });
        if (!res.ok) {
            const errData = await res.json().catch(() => ({}));
            throw new Error(errData.error || errData.message || ('HTTP ' + res.status));
        }
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.init_success') : "Initialisation terminée avec succès") || "Initialisation terminée avec succès";
            feedback.className = 'sys-feedback sys-feedback-ok';
        }
    } catch (err) {
        console.error('Serial init error:', err);
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('alert.error') : 'Erreur: ') + (err.message || err);
            feedback.className = 'sys-feedback sys-feedback-error';
        }
    } finally {
        if (icon) icon.classList.remove('animate-spin');
        if (btn) btn.disabled = false;
        if (feedback) {
            setTimeout(() => {
                feedback.innerText = '';
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
        feedback.className = 'sys-feedback sys-feedback-warn animate-pulse';
    }

    try {
        const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
        const res = await apiFetch(`/api/reset${sideQuery}`, { method: 'POST' });
        if (!res.ok) {
            const errData = await res.json().catch(() => ({}));
            throw new Error(errData.error || errData.message || ('HTTP ' + res.status));
        }
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.reset_success') : "Redémarrage demandé avec succès") || "Redémarrage demandé avec succès";
            feedback.className = 'sys-feedback sys-feedback-ok';
        }
    } catch (err) {
        console.error('Serial reset error:', err);
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('alert.error') : 'Erreur: ') + (err.message || err);
            feedback.className = 'sys-feedback sys-feedback-error';
        }
    } finally {
        if (icon) icon.classList.remove('animate-spin');
        if (btn) btn.disabled = false;
        if (feedback) {
            setTimeout(() => {
                feedback.innerText = '';
            }, 4000);
        }
    }
}

let configDomListenersAttached = false;

function initConfig() {
    if (configDomListenersAttached) return;
    configDomListenersAttached = true;

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

function updateSupervisionModeUi(mode, can_switch) {
    window.currentSupervisionMode = mode;
    const badgeActive = document.getElementById('cfgModeActiveBadge');
    const btnRemote = document.getElementById('btnModeRemote');
    const btnLocal = document.getElementById('btnModeLocal');
    const badgeRemote = document.getElementById('badgeModeRemote');
    const badgeLocal = document.getElementById('badgeModeLocal');
    const hdrModeBadge = document.getElementById('hdrModeBadge');
    const localBanner = document.getElementById('localModeBanner');

    const isLocal = mode === 'local';

    // Header badge
    if (hdrModeBadge) {
        hdrModeBadge.classList.remove('hidden');
        if (isLocal) {
            hdrModeBadge.className = 'text-[9px] font-bold px-1.5 py-0.5 rounded border font-mono mode-badge-local';
            hdrModeBadge.innerText = (typeof t === 'function' ? t('system.mode_local_short') : 'Passif') || 'Passif';
            hdrModeBadge.title = (typeof t === 'function' ? t('system.mode_local_desc') : 'Mode passif sniffer');
        } else {
            hdrModeBadge.className = 'text-[9px] font-bold px-1.5 py-0.5 rounded border font-mono mode-badge-remote';
            hdrModeBadge.innerText = (typeof t === 'function' ? t('system.mode_remote_short') : 'Supervisé') || 'Supervisé';
            hdrModeBadge.title = (typeof t === 'function' ? t('system.mode_remote_desc') : 'Mode supervisé actif');
        }
    }

    // Config Panel Buttons
    if (btnRemote && btnLocal) {
        if (isLocal) {
            btnLocal.classList.add('active', 'mode-local-active');
            btnRemote.classList.remove('active');
            if (badgeLocal) badgeLocal.classList.remove('hidden');
            if (badgeRemote) badgeRemote.classList.add('hidden');
            if (badgeActive) {
                badgeActive.className = 'sys-badge sys-badge-amber';
                badgeActive.innerText = (typeof t === 'function' ? t('system.mode_local_short') : 'Passif') || 'Passif';
            }
        } else {
            btnRemote.classList.add('active');
            btnLocal.classList.remove('active', 'mode-local-active');
            if (badgeRemote) badgeRemote.classList.remove('hidden');
            if (badgeLocal) badgeLocal.classList.add('hidden');
            if (badgeActive) {
                badgeActive.className = 'sys-badge sys-badge-emerald';
                badgeActive.innerText = (typeof t === 'function' ? t('system.mode_remote_short') : 'Supervisé') || 'Supervisé';
            }
        }

        if (can_switch !== undefined) {
            const notice = document.getElementById('cfgModeNotice');
            if (!can_switch) {
                btnRemote.disabled = true;
                btnLocal.disabled = true;
                if (notice) notice.classList.add('text-amber-400');
            } else {
                btnRemote.disabled = false;
                btnLocal.disabled = false;
                if (notice) notice.classList.remove('text-amber-400');
            }
        }
    }

    // Controls Panel Banner
    if (localBanner) {
        if (isLocal) {
            localBanner.classList.remove('hidden');
        } else {
            localBanner.classList.add('hidden');
        }
    }

    // Lock/Unlock controls in Controls panel
    if (typeof updateControlsForMode === 'function') {
        updateControlsForMode(isLocal);
    }
}

async function switchSupervisionMode(targetMode) {
    if (window.currentSupervisionMode === targetMode) return;

    if (lastReportedState && lastReportedState !== 'A' && lastReportedState !== 'DISCONNECTED') {
        alert(typeof t === 'function' ? t('system.mode_state_a_required') : "La bascule de mode nécessite que la borne soit à l'état A (Libre / Déconnectée).");
        return;
    }

    const confirmMsg = targetMode === 'local'
        ? (typeof t === 'function' ? t('system.mode_switch_confirm_local') : "Attention : le passage en mode Passif (Local) envoie une commande Reset à l'ATmega pour libérer la borne, puis cesse tout envoi de trame. Voulez-vous continuer ?")
        : (typeof t === 'function' ? t('system.mode_switch_confirm_remote') : "Voulez-vous réactiver la supervision active (mode Distant) et réinitialiser la communication avec la borne ?");

    if (!confirm(confirmMsg)) return;

    const feedback = document.getElementById('cfgModeFeedback');
    const btnRemote = document.getElementById('btnModeRemote');
    const btnLocal = document.getElementById('btnModeLocal');
    if (btnRemote) btnRemote.disabled = true;
    if (btnLocal) btnLocal.disabled = true;

    if (feedback) {
        feedback.innerText = (typeof t === 'function' ? t('system.mode_switching') : "Changement de mode en cours...") || "Changement de mode en cours...";
        feedback.className = 'sys-feedback sys-feedback-info animate-pulse';
    }

    try {
        const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
        const res = await apiFetch(`/api/mode${sideQuery}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ mode: targetMode })
        });

        if (!res.ok) {
            const errData = await res.json().catch(() => ({}));
            throw new Error(errData.error || ('HTTP ' + res.status));
        }

        const data = await res.json();
        updateSupervisionModeUi(data.mode || targetMode);

        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('system.mode_switch_success') : "Mode mis à jour avec succès") || "Mode mis à jour avec succès";
            feedback.className = 'sys-feedback sys-feedback-ok';
        }

        if (typeof fetchTelemetry === 'function') {
            fetchTelemetry();
        }
    } catch (err) {
        console.error('Mode switch error:', err);
        if (feedback) {
            feedback.innerText = (typeof t === 'function' ? t('alert.error') : 'Erreur: ') + (err.message || err);
            feedback.className = 'sys-feedback sys-feedback-error';
        }
    } finally {
        if (btnRemote) btnRemote.disabled = false;
        if (btnLocal) btnLocal.disabled = false;
        if (feedback) {
            setTimeout(() => { feedback.innerText = ''; }, 4000);
        }
    }
}
