// REST API calls to the Green'Up Link backend.

async function fetchInfo() {
    try {
        const response = await fetch('/api/info');
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
    try {
        const response = await fetch('/api/telemetry');
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
        const response = await fetch('/api/bluetooth', {
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
        await fetch('/api/tic/refresh', { method: 'POST' });
    } catch (err) {
        console.error(err);
    }
}

async function sendPostAction(url, bodyObj = null) {
    lockControls();
    try {
        let opts = { method: 'POST' };
        if (bodyObj) { opts.headers = {'Content-Type': 'application/json'}; opts.body = JSON.stringify(bodyObj); }
        await fetch(url, opts);
    } catch (err) {
        console.error(err);
    }
    setTimeout(() => {
        unlockControls();
        fetchTelemetry();
    }, 1000);
}
