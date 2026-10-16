// REST API calls and authentication management for the Green'Up Link backend.

window.isAuthRequired = false;
window.isAuthenticated = false;
let consoleInitialized = false;

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
    } else if (localStorage.getItem('greenup_debug_mode') === 'true') {
        syncLogLevelFromServer();
        startLogStream();
    }
}

async function fetchInfo() {
    if (window.isAuthRequired && !window.isAuthenticated) return;
    try {
        const response = await apiFetch('/api/info');
        if (!response.ok) return;
        const info = await response.json();

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
            document.getElementById('lblLinkVersion').innerText = `v${info.link_version}`;
        }

    } catch (err) {
        console.error("Erreur infos", err);
    }
}

async function fetchTelemetry() {
    if (window.isAuthRequired && !window.isAuthenticated) return;
    try {
        const response = await apiFetch('/api/telemetry');
        if (!response.ok) return;
        const data = await response.json();

        valVoltage.innerText = data.voltage.toFixed(1);
        valCurrent.innerText = data.current.toFixed(1);
        valPower.innerText = (data.power).toFixed(0);
        valEnergy.innerText = (data.energy).toFixed(0);

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

    } catch (err) {
        console.error("Erreur polling", err);
        lblState.innerText = t('state.offline');
        lblState.classList.add('text-rose-500');
    }
}

async function toggleBluetooth() {
    const desiredState = btToggle.checked;
    try {
        const response = await apiFetch('/api/bluetooth', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ enabled: desiredState })
        });

        if (!response.ok) {
            const data = await response.json();
            alert(t('alert.error') + data.error);
            btToggle.checked = !desiredState;
        }
        setTimeout(fetchInfo, 1000);
    } catch (error) {
        console.error(error);
        btToggle.checked = !desiredState;
    }
}

async function refreshTic() {
    if (lastReportedState !== 'A') {
        alert(t('alert.tic_refresh_state_a'));
        return;
    }
    try {
        await apiFetch('/api/tic/refresh', { method: 'POST' });
    } catch (err) {
        console.error(err);
    }
}

async function sendPostAction(url, bodyObj = null) {
    lockControls();
    try {
        let opts = { method: 'POST' };
        if (bodyObj) { opts.headers = {'Content-Type': 'application/json'}; opts.body = JSON.stringify(bodyObj); }
        await apiFetch(url, opts);
    } catch (err) {
        console.error(err);
    }
    setTimeout(() => {
        unlockControls();
        fetchTelemetry();
    }, 1000);
}
