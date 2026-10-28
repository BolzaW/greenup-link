// Configuration Panel module
// Handles viewing and saving settings (API token, TIC test, Bluetooth toggle, hardware info).

let isTokenVisible = false;

async function loadConfigData() {
    if (window.isAuthRequired && !window.isAuthenticated) return;

    try {
        const res = await apiFetch('/api/config');
        if (!res.ok) return;
        const data = await res.json();

        const tokenInput = document.getElementById('cfgApiToken');
        if (tokenInput) tokenInput.value = data.api_token || '';

        const bindElem = document.getElementById('cfgHttpBind');
        if (bindElem) bindElem.innerText = data.bind || '0.0.0.0:8080';

        const portElem = document.getElementById('cfgSerialPort');
        if (portElem) portElem.innerText = data.serial_port || '/dev/ttyUSB0';

        const baudElem = document.getElementById('cfgBaudRate');
        if (baudElem) baudElem.innerText = `${data.baud_rate || 115200} bauds`;

        // Also sync bluetooth state
        if (typeof fetchInfo === 'function') fetchInfo();
    } catch (e) {
        console.warn('Failed to load config data:', e);
    }
}

async function saveConfigData() {
    const tokenInput = document.getElementById('cfgApiToken');
    const feedback = document.getElementById('cfgSaveFeedback');
    const btnSave = document.getElementById('btnSaveConfig');
    if (!tokenInput) return;

    const newToken = tokenInput.value.trim();

    try {
        if (btnSave) btnSave.disabled = true;
        if (feedback) {
            feedback.innerText = '...';
            feedback.className = 'text-[11px] font-medium text-slate-400 text-center min-h-[16px]';
        }

        const res = await apiFetch('/api/config', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ api_token: newToken })
        });

        if (!res.ok) {
            const errData = await res.json().catch(() => ({}));
            const errMsg = errData.error || res.statusText;
            if (feedback) {
                feedback.innerText = (t('config.save_error') || 'Erreur : ') + errMsg;
                feedback.className = 'text-[11px] font-medium text-rose-400 text-center min-h-[16px]';
            }
            return;
        }

        // Update local auth token in browser to avoid unauthorized errors
        if (newToken) {
            setApiToken(newToken);
            window.isAuthRequired = true;
            window.isAuthenticated = true;
        } else {
            setApiToken('');
            window.isAuthRequired = false;
            window.isAuthenticated = true;
        }
        updateLogoutButton();

        if (feedback) {
            feedback.innerText = t('config.save_success') || 'Configuration enregistrée avec succès';
            feedback.className = 'text-[11px] font-medium text-emerald-400 text-center min-h-[16px]';
            setTimeout(() => {
                feedback.innerText = '';
            }, 3500);
        }
    } catch (e) {
        console.error('Save config error:', e);
        if (feedback) {
            feedback.innerText = (t('config.save_error') || 'Erreur : ') + e;
            feedback.className = 'text-[11px] font-medium text-rose-400 text-center min-h-[16px]';
        }
    } finally {
        if (btnSave) btnSave.disabled = false;
    }
}

function toggleTokenVisibility() {
    const input = document.getElementById('cfgApiToken');
    const iconEye = document.getElementById('iconEye');
    const iconEyeSlash = document.getElementById('iconEyeSlash');
    if (!input) return;

    isTokenVisible = !isTokenVisible;
    input.type = isTokenVisible ? 'text' : 'password';

    if (iconEye) iconEye.classList.toggle('hidden', isTokenVisible);
    if (iconEyeSlash) iconEyeSlash.classList.toggle('hidden', !isTokenVisible);
}

function copyTokenToClipboard() {
    const input = document.getElementById('cfgApiToken');
    const iconCopy = document.getElementById('iconCopyToken');
    const iconCheck = document.getElementById('iconCheckToken');
    if (!input) return;

    const val = input.value;
    if (!val) return;

    navigator.clipboard.writeText(val).then(() => {
        if (iconCopy) iconCopy.classList.add('hidden');
        if (iconCheck) iconCheck.classList.remove('hidden');
        setTimeout(() => {
            if (iconCopy) iconCopy.classList.remove('hidden');
            if (iconCheck) iconCheck.classList.add('hidden');
        }, 2000);
    }).catch(err => {
        console.warn('Clipboard write failed:', err);
    });
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
    const btnToggleVis = document.getElementById('btnToggleTokenVisibility');
    if (btnToggleVis) btnToggleVis.addEventListener('click', toggleTokenVisibility);

    const btnCopy = document.getElementById('btnCopyToken');
    if (btnCopy) btnCopy.addEventListener('click', copyTokenToClipboard);

    const btnSave = document.getElementById('btnSaveConfig');
    if (btnSave) btnSave.addEventListener('click', saveConfigData);

    const btnTic = document.getElementById('btnRefreshTicConfig');
    if (btnTic) btnTic.addEventListener('click', refreshTicFromConfig);
}
