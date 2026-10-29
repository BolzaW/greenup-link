// REST API calls and authentication management for the Green'Up Link backend.

window.isAuthRequired = false;
window.isAuthenticated = false;
window.isDualStation = false;
window.currentSide = 1;
let consoleInitialized = false;

function switchChargePoint(side) {
    if (window.currentSide === side) return;
    window.currentSide = side;

    const btn1 = document.getElementById('btnSide1');
    const btn2 = document.getElementById('btnSide2');
    if (btn1 && btn2) {
        if (side === 1) {
            btn1.classList.add('active');
            btn2.classList.remove('active');
        } else {
            btn2.classList.add('active');
            btn1.classList.remove('active');
        }
    }

    fetchInfo();
    fetchTelemetry();
    if (typeof loadSessionsData === 'function') {
        loadSessionsData();
    }
}

function getApiToken() {
    return localStorage.getItem('greenup_api_token') || '';
}

function setApiToken(token) {
    if (token) {
        localStorage.setItem('greenup_api_token', token);
    } else {
        localStorage.removeItem('greenup_api_token');
    }
}

function buildAuthUrl(url) {
    const token = getApiToken();
    if (!token) return url;
    const sep = url.includes('?') ? '&' : '?';
    return `${url}${sep}token=${encodeURIComponent(token)}`;
}

async function apiFetch(url, options = {}) {
    const token = getApiToken();
    const headers = Object.assign({}, options.headers || {});
    if (token) {
        headers['Authorization'] = `Bearer ${token}`;
    }
    const response = await fetch(url, Object.assign({}, options, { headers }));
    if (response.status === 401) {
        handleUnauthorized();
    } else if (response.status === 503) {
        try {
            const errData = await response.clone().json();
            if (errData && errData.error) {
                alert(errData.error);
            }
        } catch (_) {}
    }
    return response;
}

function handleUnauthorized() {
    window.isAuthenticated = false;
    updateLogoutButton();
    if (typeof stopLogStream === 'function') {
        stopLogStream();
    }
    showAuthModal(false);
}

function showAuthModal(showError = false) {
    const modal = document.getElementById('authModal');
    const errMsg = document.getElementById('authErrorMsg');
    const input = document.getElementById('authTokenInput');
    if (modal) modal.classList.remove('hidden');
    if (errMsg) {
        if (showError) errMsg.classList.remove('hidden');
        else errMsg.classList.add('hidden');
    }
    if (input) {
        setTimeout(() => input.focus(), 50);
    }
}

function hideAuthModal() {
    const modal = document.getElementById('authModal');
    const errMsg = document.getElementById('authErrorMsg');
    if (modal) modal.classList.add('hidden');
    if (errMsg) errMsg.classList.add('hidden');
}

function updateLogoutButton() {
    const btn = document.getElementById('btnLogout');
    if (!btn) return;
    if (window.isAuthRequired && window.isAuthenticated) {
        btn.classList.remove('hidden');
    } else {
        btn.classList.add('hidden');
    }
}

function logoutApiToken() {
    setApiToken('');
    window.isAuthenticated = false;
    updateLogoutButton();
    if (typeof stopLogStream === 'function') {
        stopLogStream();
    }
    const input = document.getElementById('authTokenInput');
    if (input) input.value = '';
    showAuthModal(false);
}

function setupAuthModalListeners() {
    const btnSubmit = document.getElementById('btnAuthSubmit');
    const input = document.getElementById('authTokenInput');
    if (btnSubmit) {
        btnSubmit.addEventListener('click', submitAuthToken);
    }
    if (input) {
        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                e.preventDefault();
                submitAuthToken();
            }
        });
    }
}

async function submitAuthToken() {
    const input = document.getElementById('authTokenInput');
    const candidate = input ? input.value.trim() : '';
    if (!candidate) {
        showAuthModal(true);
        return;
    }
    try {
        const res = await fetch('/api/auth/status', {
            headers: { 'Authorization': `Bearer ${candidate}` }
        });
        const status = await res.json();
        if (status.authenticated) {
            setApiToken(candidate);
            window.isAuthRequired = !!status.auth_required;
            window.isAuthenticated = true;
            updateLogoutButton();
            hideAuthModal();
            onAuthenticatedStart();
        } else {
            showAuthModal(true);
        }
    } catch (err) {
        console.error('Auth submission error', err);
        showAuthModal(true);
    }
}

async function checkAuthAndStart() {
    try {
        const token = getApiToken();
        const headers = token ? { 'Authorization': `Bearer ${token}` } : {};
        const res = await fetch('/api/auth/status', { headers });
        const status = await res.json();
        window.isAuthRequired = !!status.auth_required;
        window.isAuthenticated = !!status.authenticated;
        updateLogoutButton();

        if (window.isAuthRequired && !window.isAuthenticated) {
            showAuthModal(false);
        } else {
            hideAuthModal();
            onAuthenticatedStart();
        }
    } catch (err) {
        console.error('Auth status check failed', err);
        onAuthenticatedStart();
    }
}

function onAuthenticatedStart() {
    fetchTelemetry();
    fetchInfo();
    if (!consoleInitialized) {
        consoleInitialized = true;
        initConsole();
    }
    if (typeof initSessions === 'function') {
        initSessions();
    }
    if (typeof initConfig === 'function') {
        initConfig();
    }
}

async function fetchInfo() {
    if (window.isAuthRequired && !window.isAuthenticated) return;
    try {
        const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
        const response = await apiFetch(`/api/info${sideQuery}`);
        if (!response.ok) return;
        const info = await response.json();

        // Dual charge point banner visibility
        const pointBanner = document.getElementById('pointSelectorBanner');
        const wasDual = window.isDualStation;
        if (info.is_dual) {
            window.isDualStation = true;
            if (pointBanner) pointBanner.classList.remove('hidden');
        } else {
            window.isDualStation = false;
            if (pointBanner) pointBanner.classList.add('hidden');
        }
        if (wasDual !== window.isDualStation && typeof renderLogs === 'function') {
            renderLogs();
        }

        if (info.bluetooth_enabled !== null) {
            btToggle.checked = info.bluetooth_enabled;
        }

        let line1 = [];
        if (info.reference) line1.push(`Réf: ${info.reference}`);
        if (info.hardware_version) line1.push(`HW: ${info.hardware_version}`);
        if (info.software_version) line1.push(`SW: ${info.software_version}`);
        boardInfo.innerText = line1.length > 0 ? line1.join(' | ') : t('info.not_available');

        let line2 = [];
        if (info.serial_number) line2.push(`SN: ${info.serial_number}`);
        if (info.week_year_production) line2.push(`Prod: ${info.week_year_production}`);
        subInfo.innerText = line2.length > 0 ? line2.join(' | ') : '';

        if (info.link_version) {
            window.linkVersion = info.link_version;
            const lbl = document.getElementById('lblLinkVersion');
            if (lbl) lbl.innerText = `v${info.link_version}`;
            const cfgVer = document.getElementById('cfgAppVersion');
            if (cfgVer) cfgVer.innerText = `v${info.link_version}`;
        }

    } catch (err) {
        console.error("Erreur infos", err);
    }
}


function updateUsbStatus(serial) {
    if (!serial) return;
    const status = serial.status;
    window.isUsbConnected = (status === 'usb_connected');

    const usbLed = document.getElementById('usbLed');
    const usbStatusBadge = document.getElementById('usbStatusBadge');
    const usbAlertBanner = document.getElementById('usbAlertBanner');
    const usbAlertIcon = document.getElementById('usbAlertIcon');
    const usbAlertText = document.getElementById('usbAlertText');
    const controlsPanel = document.getElementById('controlsPanel');

    // Pastille d'alerte sur l'onglet Système
    const tabSystemBadge = document.getElementById('tabSystemBadge');
    const tabSystemBadgePing = document.getElementById('tabSystemBadgePing');
    const tabSystemBadgeDot = document.getElementById('tabSystemBadgeDot');

    // Détails USB et ports dans le panneau Système
    const sysUsbDetails = document.getElementById('sysUsbDetails');
    const cfgSerialPort = document.getElementById('cfgSerialPort');
    const cfgBaudRate = document.getElementById('cfgBaudRate');

    if (cfgSerialPort && serial.port) {
        cfgSerialPort.innerText = serial.port;
    }
    if (cfgBaudRate && serial.baud_rate) {
        cfgBaudRate.innerText = `${serial.baud_rate} bauds`;
    }

    if (!usbLed || !usbStatusBadge || !usbAlertBanner) return;

    // Base styling
    usbLed.className = 'w-2.5 h-2.5 rounded-full inline-block flex-shrink-0';
    usbStatusBadge.className = 'font-bold text-xs flex-shrink-0';
    usbAlertBanner.className = 'w-full mb-3 p-3 px-4 rounded-xl border flex items-center space-x-3 transition-all duration-300';

    if (status === 'usb_connected') {
        usbLed.classList.add('bg-emerald-500', 'shadow-[0_0_8px_rgba(16,185,129,0.8)]');
        usbStatusBadge.classList.add('text-emerald-400');
        usbStatusBadge.innerText = t('usb.status_connected');
        usbAlertBanner.classList.add('hidden');
        if (controlsPanel && !isActionPending) {
            controlsPanel.classList.remove('opacity-40', 'pointer-events-none');
            controlsPanel.removeAttribute('title');
        }

        // Hide alert badge on System tab
        if (tabSystemBadge) tabSystemBadge.classList.add('hidden');

        if (sysUsbDetails) {
            const rxInfo = serial.last_rx_secs_ago != null ? ` (dernier échange : ${serial.last_rx_secs_ago}s)` : '';
            sysUsbDetails.innerText = (serial.message || 'Liaison USB active avec la carte ATmega') + rxInfo;
            sysUsbDetails.className = 'text-[11px] font-mono text-emerald-400/80 pl-1 truncate';
        }
    } else if (status === 'usb_connecting') {
        usbLed.classList.add('bg-amber-400', 'animate-pulse');
        usbStatusBadge.classList.add('text-amber-400');
        usbStatusBadge.innerText = t('usb.status_connecting');
        usbAlertBanner.classList.remove('hidden');
        usbAlertBanner.classList.add('bg-amber-950/40', 'border-amber-700/60', 'text-amber-200');
        if (usbAlertIcon) usbAlertIcon.innerText = '⏳';
        if (usbAlertText) usbAlertText.innerText = t('usb.alert_connecting');
        if (controlsPanel && !isActionPending) {
            controlsPanel.classList.add('opacity-40', 'pointer-events-none');
            controlsPanel.title = t('usb.controls_disabled');
        }

        // Show amber alert pastille on System tab
        if (tabSystemBadge) tabSystemBadge.classList.remove('hidden');
        if (tabSystemBadgePing) tabSystemBadgePing.className = 'animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-75';
        if (tabSystemBadgeDot) tabSystemBadgeDot.className = 'relative inline-flex rounded-full h-2.5 w-2.5 bg-amber-500 border border-slate-900';

        if (sysUsbDetails) {
            sysUsbDetails.innerText = serial.message || t('usb.alert_connecting');
            sysUsbDetails.className = 'text-[11px] font-mono text-amber-400/80 pl-1 truncate';
        }
    } else if (status === 'usb_no_response') {
        usbLed.classList.add('bg-amber-500', 'animate-pulse');
        usbStatusBadge.classList.add('text-amber-400');
        usbStatusBadge.innerText = t('usb.status_no_response');
        usbAlertBanner.classList.remove('hidden');
        usbAlertBanner.classList.add('bg-amber-950/50', 'border-amber-600', 'text-amber-200');
        if (usbAlertIcon) usbAlertIcon.innerText = '⚠️';
        if (usbAlertText) usbAlertText.innerText = t('usb.alert_no_response');
        if (controlsPanel && !isActionPending) {
            controlsPanel.classList.add('opacity-40', 'pointer-events-none');
            controlsPanel.title = t('usb.controls_disabled');
        }

        // Show amber alert pastille on System tab
        if (tabSystemBadge) tabSystemBadge.classList.remove('hidden');
        if (tabSystemBadgePing) tabSystemBadgePing.className = 'animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-75';
        if (tabSystemBadgeDot) tabSystemBadgeDot.className = 'relative inline-flex rounded-full h-2.5 w-2.5 bg-amber-500 border border-slate-900';

        if (sysUsbDetails) {
            sysUsbDetails.innerText = serial.message || t('usb.alert_no_response');
            sysUsbDetails.className = 'text-[11px] font-mono text-amber-400/80 pl-1 truncate';
        }
    } else {
        // usb_disconnected
        usbLed.classList.add('bg-rose-500', 'animate-pulse');
        usbStatusBadge.classList.add('text-rose-400');
        usbStatusBadge.innerText = t('usb.status_disconnected');
        usbAlertBanner.classList.remove('hidden');
        usbAlertBanner.classList.add('bg-rose-950/50', 'border-rose-700/70', 'text-rose-200');
        if (usbAlertIcon) usbAlertIcon.innerText = '🔴';
        if (usbAlertText) usbAlertText.innerText = t('usb.alert_disconnected');
        if (controlsPanel && !isActionPending) {
            controlsPanel.classList.add('opacity-40', 'pointer-events-none');
            controlsPanel.title = t('usb.controls_disabled');
        }

        // Show red alert pastille on System tab
        if (tabSystemBadge) tabSystemBadge.classList.remove('hidden');
        if (tabSystemBadgePing) tabSystemBadgePing.className = 'animate-ping absolute inline-flex h-full w-full rounded-full bg-rose-400 opacity-75';
        if (tabSystemBadgeDot) tabSystemBadgeDot.className = 'relative inline-flex rounded-full h-2.5 w-2.5 bg-rose-500 border border-slate-900';

        if (sysUsbDetails) {
            sysUsbDetails.innerText = serial.message || t('usb.alert_disconnected');
            sysUsbDetails.className = 'text-[11px] font-mono text-rose-400/80 pl-1 truncate';
        }
    }
}

async function fetchTelemetry() {
    if (window.isAuthRequired && !window.isAuthenticated) return;
    try {
        const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
        const response = await apiFetch(`/api/telemetry${sideQuery}`);
        if (!response.ok) return;
        const data = await response.json();

        if (data.serial) {
            updateUsbStatus(data.serial);
        }

        if (!window.isUsbConnected) {
            valVoltage.innerText = '--';
            valCurrent.innerText = '--';
            valPower.innerText = '--';
            valEnergy.innerText = '--';
        } else {
            valVoltage.innerText = data.voltage.toFixed(1);
            valCurrent.innerText = data.current.toFixed(1);
            valPower.innerText = (data.power).toFixed(0);
            valEnergy.innerText = (data.energy).toFixed(0);
        }

        if (data.error_code && data.error_code !== "0000" && data.error_code !== "") {
            const desc = t('hardware_errors.' + data.error_code);
            const descText = (desc && desc !== 'hardware_errors.' + data.error_code) ? ` - ${desc}` : '';
            lblError.innerText = `${t('error.hardware_code')}${data.error_code}${descText}`;
        } else {
            lblError.innerText = "";
        }

        if (data.eliot_limit_amps && !isDraggingSlider && !isActionPending) {
            slider.value = data.eliot_limit_amps;
            lblSlider.innerText = data.eliot_limit_amps + " A";
        }

        lastReportedState = data.greenup_state;
        updateLinky(data.tic_mode);
        updateVisuals(data.greenup_state, data.charge_complete, data.iec_state, data.error_code, data.charge_paused, data.charge_authorized);

        if (typeof onTelemetryUpdateForSessions === 'function') {
            onTelemetryUpdateForSessions(data);
        }

    } catch (err) {
        console.error("Erreur polling", err);
        lblState.innerText = t('state.offline');
        lblState.classList.add('text-rose-500');
    }
}

async function toggleBluetooth() {
    const btToggle = document.getElementById('btToggle');
    const btFeedback = document.getElementById('btToggleFeedback');
    if (!btToggle) return;
    const desiredState = btToggle.checked;
    btToggle.disabled = true;
    if (btFeedback) {
        btFeedback.innerText = (typeof t === 'function' ? t('system.ble_pending') : 'Commutation en cours...') || 'Commutation en cours...';
        btFeedback.className = 'text-[10px] font-medium text-sky-400 min-h-[14px] mt-2 animate-pulse';
    }
    try {
        const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
        const response = await apiFetch(`/api/bluetooth${sideQuery}`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ enabled: desiredState })
        });

        if (!response.ok) {
            const data = await response.json();
            alert(t('alert.error') + (data.error || 'Erreur'));
            btToggle.checked = !desiredState;
        }
        setTimeout(fetchInfo, 1000);
    } catch (error) {
        console.error(error);
        btToggle.checked = !desiredState;
    } finally {
        btToggle.disabled = false;
        if (btFeedback) {
            setTimeout(() => {
                btFeedback.innerText = '';
                btFeedback.className = 'text-[10px] font-medium text-sky-400 min-h-[14px] mt-2';
            }, 2000);
        }
    }
}

async function refreshTic() {
    if (lastReportedState !== 'A') {
        alert(t('alert.tic_refresh_state_a'));
        return;
    }
    try {
        const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
        await apiFetch(`/api/tic/refresh${sideQuery}`, { method: 'POST' });
    } catch (err) {
        console.error(err);
    }
}

async function sendPostAction(url, bodyObj = null) {
    if (!window.isUsbConnected) {
        alert(t('usb.controls_disabled'));
        return;
    }
    lockControls();
    try {
        let reqUrl = url;
        if (window.currentSide && !reqUrl.includes('side=')) {
            const sep = reqUrl.includes('?') ? '&' : '?';
            reqUrl = `${reqUrl}${sep}side=${window.currentSide}`;
        }
        let opts = { method: 'POST' };
        if (bodyObj) { opts.headers = {'Content-Type': 'application/json'}; opts.body = JSON.stringify(bodyObj); }
        await apiFetch(reqUrl, opts);
    } catch (err) {
        console.error(err);
    }
    setTimeout(() => {
        unlockControls();
        fetchTelemetry();
    }, 1000);
}