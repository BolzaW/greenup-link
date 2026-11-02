// Sessions Management & Power Curve Visualization module
// Handles tab switching, session listing, metrics display, CSV chart rendering, ZIP export, and deletion.

let sessionsCache = { active_session: null, sessions: [] };
let selectedSessionId = null;
let sessionCsvCache = new Map();
let currentChartPoints = [];
let deleteConfirmTimer = null;
let sessionMetricMode = localStorage.getItem('greenup_session_metric_mode') || 'power';

function formatSessionDuration(totalSeconds, includeSeconds = true) {
    if (totalSeconds == null || isNaN(totalSeconds)) return '--';
    const s = Math.max(0, Math.floor(totalSeconds));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    const hLabel = t('sessions.time_h') || 'h';
    const mLabel = t('sessions.time_m') || 'm';
    const sLabel = t('sessions.time_s') || 's';

    if (h > 0) {
        if (!includeSeconds) {
            return `${h}${hLabel} ${m.toString().padStart(2, '0')}${mLabel}`;
        }
        return `${h}${hLabel} ${m.toString().padStart(2, '0')}${mLabel} ${sec.toString().padStart(2, '0')}${sLabel}`;
    }
    if (m > 0) {
        if (!includeSeconds) {
            return `${m}${mLabel}`;
        }
        return `${m}${mLabel} ${sec.toString().padStart(2, '0')}${sLabel}`;
    }
    return `${sec}${sLabel}`;
}

function formatChartTimeLabel(totalSeconds) {
    const s = Math.max(0, Math.floor(totalSeconds));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const hLabel = t('sessions.time_h') || 'h';
    const mLabel = t('sessions.time_m') || 'm';

    if (h > 0) {
        return m > 0 ? `${h}${hLabel}${m.toString().padStart(2, '0')}` : `${h}${hLabel}`;
    }
    return `${m}${mLabel}`;
}

function formatSessionState(st, iecState) {
    if (!st) return '--';
    const s = st.toUpperCase();
    if (s === 'D' || s === 'E' || iecState === 'C') {
        return `${t('sessions.state_charging') || 'En charge'} (${s})`;
    } else if (s === 'C') {
        return `${t('sessions.state_waiting_car') || 'Attente véhicule'} (${s})`;
    } else if (s === 'B') {
        return `${t('sessions.state_waiting_station') || 'Attente borne'} (${s})`;
    } else if (s === 'I') {
        return `${t('sessions.state_complete') || 'Charge terminée (Veille)'} (${s})`;
    } else if (s === 'A') {
        return `${t('sessions.state_disconnected') || 'Débranché'} (${s})`;
    } else if (s === 'L') {
        return `${t('sessions.state_unlatched') || 'Câble déverrouillé'} (${s})`;
    } else if (['R', 'V', 'X', 'ST', 'S', 'T', 'U', 'Y', 'Z'].includes(s) || iecState === 'E' || iecState === 'F') {
        return `${t('sessions.state_fault') || 'Défaut borne'} (${s})`;
    }
    return `${t('state.connected') || 'Connecté'} (${s})`;
}

function formatTerminationReason(reason) {
    if (!reason || reason === '--') return '--';
    const r = reason.trim();
    if (r.includes('State: A') || r.toLowerCase().includes('state: a') || r.toLowerCase().includes('disconnected')) {
        return t('sessions.reason_unplugged') || 'Véhicule débranché';
    }
    if (r.includes('State: L') || r.toLowerCase().includes('unlocked') || r.toLowerCase().includes('unlatched')) {
        return t('sessions.reason_unlatched') || 'Câble déverrouillé';
    }
    if (r.toLowerCase().includes('fault') || r.toLowerCase().includes('safety')) {
        const match = r.match(/State:\s*([A-Za-z0-9_-]+)/i);
        const codeSuffix = match ? ` (${match[1]})` : '';
        return `${t('sessions.reason_fault') || 'Défaut borne'}${codeSuffix}`;
    }
    if (r.toLowerCase().includes('shutdown')) {
        return t('sessions.reason_shutdown') || 'Arrêt liaison / service';
    }
    if (r.toLowerCase().includes('superseded')) {
        return t('sessions.reason_superseded') || 'Nouvelle session détectée';
    }
    return r;
}

// ==============================================================================
// TAB SWITCHING LOGIC (Board / Sessions / Console)
// ==============================================================================

function switchMainTab(tabName) {
    const visualizerPanel = document.getElementById('visualizerPanel');
    const sessionsPanel = document.getElementById('sessionsPanel');
    const consolePanel = document.getElementById('consolePanel');
    const systemPanel = document.getElementById('systemPanel') || document.getElementById('configPanel');
    const tabBoard = document.getElementById('tabBoard');
    const tabSessions = document.getElementById('tabSessions');
    const tabConsole = document.getElementById('tabConsole');
    const tabSystem = document.getElementById('tabSystem') || document.getElementById('tabConfig');

    [tabBoard, tabSessions, tabConsole, tabSystem].forEach(btn => {
        if (btn) btn.classList.remove('active');
    });

    if (visualizerPanel) visualizerPanel.classList.add('hidden');
    if (sessionsPanel) sessionsPanel.classList.add('hidden');
    if (consolePanel) consolePanel.classList.add('hidden');
    if (systemPanel) systemPanel.classList.add('hidden');

    if (tabName === 'config') tabName = 'system';

    if (tabName === 'sessions') {
        if (tabSessions) tabSessions.classList.add('active');
        if (sessionsPanel) sessionsPanel.classList.remove('hidden');
        if (typeof stopLogStream === 'function') stopLogStream();
        loadSessionsData(true);
        // Trigger chart resize after layout
        setTimeout(resizeAndRedrawChart, 60);
    } else if (tabName === 'console') {
        if (tabConsole) tabConsole.classList.add('active');
        if (consolePanel) consolePanel.classList.remove('hidden');
        if (typeof syncLogLevelFromServer === 'function') syncLogLevelFromServer();
        if (typeof startLogStream === 'function') startLogStream();
    } else if (tabName === 'system') {
        if (tabSystem) tabSystem.classList.add('active');
        if (systemPanel) systemPanel.classList.remove('hidden');
        if (typeof stopLogStream === 'function') stopLogStream();
        if (typeof loadConfigData === 'function') loadConfigData();
    } else {
        // default to 'board'
        tabName = 'board';
        if (tabBoard) tabBoard.classList.add('active');
        if (visualizerPanel) visualizerPanel.classList.remove('hidden');
        if (typeof stopLogStream === 'function') stopLogStream();
    }

    localStorage.setItem('greenup_active_tab', tabName);
}

// ==============================================================================
// SESSIONS DATA & UI MANAGEMENT
// ==============================================================================

async function loadSessionsData(isDefaultSelectionRequested = false) {
    if (!window.canReadData || !window.canReadData()) return;

    try {
        const sideQuery = window.currentSide ? `?side=${window.currentSide}` : '';
        const response = await apiFetch(`/api/sessions${sideQuery}`);
        if (!response.ok) return;
        const data = await response.json();
        sessionsCache = data || { active_session: null, sessions: [] };
        updateSessionsDropdown(isDefaultSelectionRequested);
    } catch (e) {
        console.error('Failed to load sessions data:', e);
    }
}

function updateSessionsDropdown(isDefaultSelectionRequested) {
    const select = document.getElementById('sessionSelect');
    if (!select) return;

    const previousSelected = selectedSessionId;
    select.innerHTML = '';

    const hasActive = !!(sessionsCache && sessionsCache.active_session);
    const savedList = (sessionsCache && Array.isArray(sessionsCache.sessions)) ? sessionsCache.sessions : [];

    if (!hasActive && savedList.length === 0) {
        const opt = document.createElement('option');
        opt.value = '';
        opt.disabled = true;
        opt.selected = true;
        opt.setAttribute('data-i18n', 'sessions.no_sessions');
        opt.innerText = t('sessions.no_sessions') || 'Aucune session enregistrée';
        select.appendChild(opt);
        renderEmptySessionView();
        return;
    }

    // 1. Active session option (if currently active)
    if (hasActive) {
        const act = sessionsCache.active_session;
        const opt = document.createElement('option');
        opt.value = 'active';
        const prefix = t('sessions.active_prefix') || '⚡ [En cours]';
        opt.innerText = `${prefix} ${act.start_time} — ${act.total_energy_kwh.toFixed(2)} kWh (${formatSessionDuration(act.charging_duration_seconds, false)})`;
        select.appendChild(opt);
    }

    // 2. Saved sessions list
    savedList.forEach(s => {
        const opt = document.createElement('option');
        opt.value = s.id;
        const sideBadge = (window.isDualStation && s.side) ? `[P${s.side}] ` : '';
        opt.innerText = `📅 ${sideBadge}${s.start_time} — ${s.total_energy_kwh.toFixed(2)} kWh (${formatSessionDuration(s.charging_duration_seconds, false)})`;
        select.appendChild(opt);
    });

    // 3. Selection determination
    // Rule requested by user:
    // "Pour le sélectionneur, on sélectionne par défaut la session en cours, et sinon, la session la plus récente."
    let targetSelection = null;
    if (!isDefaultSelectionRequested && previousSelected) {
        // Check if previousSelected still exists
        if (previousSelected === 'active' && hasActive) {
            targetSelection = 'active';
        } else if (savedList.some(s => s.id === previousSelected)) {
            targetSelection = previousSelected;
        }
    }

    if (!targetSelection) {
        if (hasActive) {
            targetSelection = 'active';
        } else if (savedList.length > 0) {
            targetSelection = savedList[0].id;
        }
    }

    if (targetSelection) {
        select.value = targetSelection;
        onSessionSelected(targetSelection);
    } else {
        renderEmptySessionView();
    }
}

function renderMetricCard() {
    const act = (selectedSessionId === 'active') ? sessionsCache.active_session : null;
    const savedList = sessionsCache.sessions || [];
    const s = act || savedList.find(item => item.id === selectedSessionId);

    const titleEl = document.getElementById('sessionMetricTitle');
    const maxValEl = document.getElementById('sessionValMaxPower');
    const avgValEl = document.getElementById('sessionValAvgPower');
    const iconEl = document.getElementById('sessionMetricIcon');

    if (!s) {
        if (maxValEl) maxValEl.innerText = '--';
        if (avgValEl) avgValEl.innerText = '--';
        return;
    }

    if (sessionMetricMode === 'current') {
        if (titleEl) {
            titleEl.setAttribute('data-i18n', 'sessions.stat_current_max');
            titleEl.innerText = t('sessions.stat_current_max') || 'Courant';
        }
        if (iconEl) iconEl.innerText = '⚡';
        if (maxValEl) {
            maxValEl.className = 'text-sm sm:text-base font-bold text-sky-400 font-mono mt-1';
            const maxA = s.max_current_a != null ? s.max_current_a : 0;
            maxValEl.innerText = `${maxA.toFixed(1)} A`;
        }
        if (avgValEl) {
            const avgA = (s.avg_current_a != null && s.avg_current_a > 0)
                ? s.avg_current_a
                : (s.avg_power_w && s.avg_power_w > 0 ? (s.avg_power_w / 230) : 0);
            avgValEl.innerText = avgA > 0 ? `Moy: ${avgA.toFixed(1)} A` : 'Moy: --';
        }
    } else {
        // power mode
        if (titleEl) {
            titleEl.setAttribute('data-i18n', 'sessions.stat_power_max');
            titleEl.innerText = t('sessions.stat_power_max') || 'Puissance';
        }
        if (iconEl) iconEl.innerText = '📈';
        if (maxValEl) {
            maxValEl.className = 'text-sm sm:text-base font-bold text-emerald-400 font-mono mt-1';
            const maxW = s.max_power_w != null ? s.max_power_w : 0;
            maxValEl.innerText = `${(maxW / 1000).toFixed(2)} kW`;
        }
        if (avgValEl) {
            const avgW = (s.avg_power_w != null && s.avg_power_w > 0) ? s.avg_power_w : 0;
            avgValEl.innerText = avgW > 0 ? `Moy: ${(avgW / 1000).toFixed(2)} kW` : 'Moy: --';
        }
    }
}

function setSessionMetricMode(mode) {
    if (mode !== 'current' && mode !== 'power') mode = 'power';
    sessionMetricMode = mode;
    localStorage.setItem('greenup_session_metric_mode', mode);

    const toggleKw = document.getElementById('toggleKwLabel');
    const toggleAmps = document.getElementById('toggleAmpsLabel');
    if (toggleKw) toggleKw.classList.toggle('active', mode === 'power');
    if (toggleAmps) toggleAmps.classList.toggle('active', mode === 'current');

    const btnKw = document.getElementById('btnChartModeKw');
    const btnAmps = document.getElementById('btnChartModeAmps');
    if (btnKw) btnKw.classList.toggle('active', mode === 'power');
    if (btnAmps) {
        btnAmps.classList.toggle('active', mode === 'current');
        btnAmps.classList.toggle('amps-mode', mode === 'current');
    }

    const chartTitle = document.getElementById('sessionChartTitle');
    const chartDot = document.getElementById('sessionChartDot');
    if (chartTitle) {
        const key = mode === 'current' ? 'sessions.chart_current_curve' : 'sessions.chart_power_curve';
        chartTitle.setAttribute('data-i18n', key);
        chartTitle.innerText = t(key) || (mode === 'current' ? 'Courbe de courant' : 'Courbe de puissance');
    }
    if (chartDot) {
        if (mode === 'current') {
            chartDot.classList.remove('bg-emerald-400');
            chartDot.classList.add('bg-sky-400');
        } else {
            chartDot.classList.remove('bg-sky-400');
            chartDot.classList.add('bg-emerald-400');
        }
    }

    renderMetricCard();
    drawChart(currentChartPoints);
}

function renderEmptySessionView() {
    selectedSessionId = null;
    currentChartPoints = [];
    resetDeleteButton();

    const btnZip = document.getElementById('btnSessionDownloadZip');
    const btnDel = document.getElementById('btnSessionDelete');
    if (btnZip) btnZip.disabled = true;
    if (btnDel) btnDel.disabled = true;

    setElemText('sessionValChargingDuration', '--');
    setElemText('sessionValTotalDuration', '--');
    setElemText('sessionValEnergy', '--');
    setElemText('sessionValPhase', '--');
    renderMetricCard();
    const reasonEl = document.getElementById('sessionValReason');
    if (reasonEl) {
        reasonEl.setAttribute('data-i18n', 'sessions.no_sessions');
        reasonEl.innerText = t('sessions.no_sessions') || 'Aucune session enregistrée';
        reasonEl.title = '';
    }
    setElemText('sessionValEndTime', '--');

    drawChart([]);
}

function onSessionSelected(id) {
    selectedSessionId = id;
    resetDeleteButton();

    const btnZip = document.getElementById('btnSessionDownloadZip');
    const btnDel = document.getElementById('btnSessionDelete');

    if (id === 'active') {
        const act = sessionsCache.active_session;
        if (!act) return;

        // Active session metrics
        setElemText('sessionValChargingDuration', formatSessionDuration(act.charging_duration_seconds));
        setElemText('sessionValTotalDuration', `Total: ${formatSessionDuration(act.elapsed_seconds)}`);
        setElemText('sessionValEnergy', `${act.total_energy_kwh.toFixed(2)} kWh`);
        const plug = act.plug_type || 'Type 2';
        const phase = act.phase || 'Mono';
        setElemText('sessionValPhase', `${plug} • ${phase}`);

        renderMetricCard();

        const reasonEl = document.getElementById('sessionValReason');
        if (reasonEl) {
            reasonEl.removeAttribute('data-i18n');
            reasonEl.innerText = formatSessionState(act.state, act.iec_state);
            const iecPart = act.iec_state ? ` • IEC 61851: État ${act.iec_state}` : '';
            reasonEl.title = `État Legrand: ${act.state}${iecPart}`;
        }
        // Supprime toute référence aux données instantanées
        setElemText('sessionValEndTime', t('sessions.active_ongoing') || 'Session en cours');

        if (btnZip) btnZip.disabled = false;
        if (btnDel) {
            btnDel.disabled = true;
            btnDel.title = t('sessions.delete_disabled_active') || "Impossible de supprimer la session en cours de charge";
        }

        // Fetch active CSV data for live chart
        loadAndDisplayChart(act.session_id);
    } else {
        const savedList = sessionsCache.sessions || [];
        const s = savedList.find(item => item.id === id);
        if (!s) return;

        setElemText('sessionValChargingDuration', formatSessionDuration(s.charging_duration_seconds));
        setElemText('sessionValTotalDuration', `Total: ${formatSessionDuration(s.total_duration_seconds)}`);
        setElemText('sessionValEnergy', `${s.total_energy_kwh.toFixed(2)} kWh`);
        const plug = s.plug_type || 'Type 2';
        const phase = s.phase || 'Mono';
        setElemText('sessionValPhase', `${plug} • ${phase}`);

        renderMetricCard();

        const reasonElem = document.getElementById('sessionValReason');
        if (reasonElem) {
            reasonElem.removeAttribute('data-i18n');
            reasonElem.innerText = formatTerminationReason(s.termination_reason);
            reasonElem.title = s.termination_reason || '';
        }
        setElemText('sessionValEndTime', s.end_time ? `Fin: ${s.end_time}` : '--');

        if (btnZip) btnZip.disabled = !s.has_csv && !s.has_txt;
        if (btnDel) {
            btnDel.disabled = false;
            btnDel.title = t('sessions.delete_title') || 'Supprimer la session';
        }

        loadAndDisplayChart(s.id);
    }
}

function setElemText(id, text) {
    const el = document.getElementById(id);
    if (el) el.innerText = text;
}

// ==============================================================================
// CSV LOADING & CHART RENDERING
// ==============================================================================

async function loadAndDisplayChart(sessionId) {
    const statusElem = document.getElementById('sessionChartStatus');
    if (statusElem && currentChartPoints.length === 0) {
        statusElem.innerText = t('sessions.chart_loading') || 'Chargement...';
    }

    if (sessionCsvCache.has(sessionId)) {
        const cachedPoints = sessionCsvCache.get(sessionId);
        currentChartPoints = cachedPoints;
        drawChart(cachedPoints);
        if (statusElem) statusElem.innerText = `${cachedPoints.length} pts`;
        return;
    }

    try {
        const res = await apiFetch(`/api/sessions/${encodeURIComponent(sessionId)}/export?format=csv`);

        if (!res.ok) {
            currentChartPoints = [];
            drawChart([]);
            if (statusElem) statusElem.innerText = '';
            return;
        }

        const csvText = await res.text();
        const points = parseCsvPoints(csvText);

        // Cache completed sessions (do not cache active session permanently so it updates)
        if (selectedSessionId !== 'active') {
            sessionCsvCache.set(sessionId, points);
        }

        currentChartPoints = points;
        drawChart(points);
        if (statusElem) statusElem.innerText = `${points.length} pts`;
    } catch (e) {
        console.warn('Could not load session CSV for chart:', e);
        currentChartPoints = [];
        drawChart([]);
        if (statusElem) statusElem.innerText = '';
    }
}

function parseCsvPoints(csvText) {
    const lines = csvText.trim().split('\n');
    if (lines.length < 2) return [];

    const headers = lines[0].split(',').map(h => h.trim().toLowerCase());
    let elapsedIdx = headers.indexOf('elapsed_s');
    if (elapsedIdx === -1) elapsedIdx = headers.indexOf('elapsed');
    let powerIdx = headers.indexOf('power_w');
    if (powerIdx === -1) powerIdx = headers.indexOf('power');
    let currentIdx = headers.indexOf('current_a');
    if (currentIdx === -1) currentIdx = headers.indexOf('current');
    let stateIdx = headers.indexOf('state');

    if (elapsedIdx === -1 || powerIdx === -1) return [];

    const points = [];
    for (let i = 1; i < lines.length; i++) {
        const line = lines[i].trim();
        if (!line) continue;
        const cols = line.split(',');
        const elapsed = parseFloat(cols[elapsedIdx]) || 0;
        const powerW = parseFloat(cols[powerIdx]) || 0;
        const currentA = (currentIdx !== -1 && cols[currentIdx]) ? parseFloat(cols[currentIdx]) || 0 : 0;
        const state = (stateIdx !== -1 && cols[stateIdx]) ? cols[stateIdx].trim() : '';

        points.push({
            t: elapsed,
            kw: Math.max(0, powerW / 1000.0),
            amps: Math.max(0, currentA),
            state: state
        });
    }

    return points;
}

// Native Canvas Drawing with High DPI & Hover interactions
let chartCanvasGeometry = { width: 0, height: 0, padLeft: 42, padRight: 15, padTop: 18, padBottom: 26 };

function resizeAndRedrawChart() {
    const canvas = document.getElementById('sessionChartCanvas');
    if (!canvas) return;

    const rect = canvas.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return;

    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(rect.width * dpr);
    canvas.height = Math.round(rect.height * dpr);

    const ctx = canvas.getContext('2d');
    ctx.scale(dpr, dpr);

    chartCanvasGeometry.width = rect.width;
    chartCanvasGeometry.height = rect.height;

    drawChart(currentChartPoints);
}

function drawChart(points, hoverIndex = -1) {
    const canvas = document.getElementById('sessionChartCanvas');
    const emptyElem = document.getElementById('sessionChartEmpty');
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    const { width, height, padLeft, padRight, padTop, padBottom } = chartCanvasGeometry;

    if (width === 0 || height === 0) return;

    ctx.clearRect(0, 0, width, height);

    if (!points || points.length === 0) {
        if (emptyElem) emptyElem.classList.remove('hidden');
        return;
    }
    if (emptyElem) emptyElem.classList.add('hidden');

    const plotW = Math.max(10, width - padLeft - padRight);
    const plotH = Math.max(10, height - padTop - padBottom);

    const maxT = Math.max(points[points.length - 1].t, 60);
    const isCurrentMode = sessionMetricMode === 'current';

    let maxVal = 0;
    for (let p of points) {
        const val = isCurrentMode ? p.amps : p.kw;
        if (val > maxVal) maxVal = val;
    }

    let yMax;
    if (isCurrentMode) {
        yMax = Math.ceil(Math.max(maxVal * 1.15, 16));
        if (yMax > 16 && yMax <= 32) yMax = 32;
        else if (yMax > 32 && yMax <= 40) yMax = 40;
    } else {
        yMax = Math.ceil(Math.max(maxVal * 1.15, 3.7));
    }

    // Coordinates mapping
    const getX = (t) => padLeft + (t / maxT) * plotW;
    const getY = (val) => padTop + plotH - (val / yMax) * plotH;

    // 1. Draw horizontal grid lines & Y labels (3 divisions)
    ctx.lineWidth = 1;
    ctx.font = '10px ui-monospace, SFMono-Regular, monospace';
    ctx.textAlign = 'right';
    ctx.textBaseline = 'middle';

    const ySteps = 3;
    for (let i = 0; i <= ySteps; i++) {
        const stepVal = (yMax / ySteps) * i;
        const y = getY(stepVal);

        ctx.strokeStyle = (i === 0) ? '#334155' : 'rgba(51, 65, 85, 0.4)';
        ctx.beginPath();
        ctx.moveTo(padLeft, y);
        ctx.lineTo(width - padRight, y);
        ctx.stroke();

        ctx.fillStyle = '#64748b';
        const labelText = isCurrentMode ? `${Math.round(stepVal)} A` : `${stepVal.toFixed(1)}k`;
        ctx.fillText(labelText, padLeft - 6, y);
    }

    // 2. Draw X axis time labels (4 intervals)
    ctx.textAlign = 'center';
    ctx.textBaseline = 'top';
    const xSteps = 4;
    for (let i = 0; i <= xSteps; i++) {
        const tVal = (maxT / xSteps) * i;
        const x = getX(tVal);
        ctx.fillStyle = '#64748b';
        ctx.fillText(formatChartTimeLabel(tVal), x, height - padBottom + 6);
    }

    // 3. Draw gradient filled area under the curve
    const gradient = ctx.createLinearGradient(0, padTop, 0, padTop + plotH);
    if (isCurrentMode) {
        gradient.addColorStop(0, 'rgba(14, 165, 233, 0.35)');
        gradient.addColorStop(0.7, 'rgba(56, 189, 248, 0.15)');
        gradient.addColorStop(1, 'rgba(15, 23, 42, 0.0)');
    } else {
        gradient.addColorStop(0, 'rgba(16, 185, 129, 0.35)');
        gradient.addColorStop(0.7, 'rgba(14, 165, 233, 0.15)');
        gradient.addColorStop(1, 'rgba(15, 23, 42, 0.0)');
    }

    ctx.beginPath();
    ctx.moveTo(getX(points[0].t), getY(0));
    for (let i = 0; i < points.length; i++) {
        const val = isCurrentMode ? points[i].amps : points[i].kw;
        ctx.lineTo(getX(points[i].t), getY(val));
    }
    ctx.lineTo(getX(points[points.length - 1].t), getY(0));
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();

    // 4. Draw curve line
    ctx.beginPath();
    ctx.strokeStyle = isCurrentMode ? '#0ea5e9' : '#10b981';
    ctx.lineWidth = 2;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    for (let i = 0; i < points.length; i++) {
        const x = getX(points[i].t);
        const y = getY(isCurrentMode ? points[i].amps : points[i].kw);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // 5. Draw hover marker and crosshair if requested
    if (hoverIndex >= 0 && hoverIndex < points.length) {
        const p = points[hoverIndex];
        const hx = getX(p.t);
        const hy = getY(isCurrentMode ? p.amps : p.kw);

        // Vertical dashed line
        ctx.setLineDash([3, 3]);
        ctx.strokeStyle = isCurrentMode ? 'rgba(56, 189, 248, 0.8)' : 'rgba(52, 211, 153, 0.8)';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(hx, padTop);
        ctx.lineTo(hx, padTop + plotH);
        ctx.stroke();
        ctx.setLineDash([]);

        // Outer glow
        ctx.beginPath();
        ctx.arc(hx, hy, 6, 0, Math.PI * 2);
        ctx.fillStyle = isCurrentMode ? 'rgba(14, 165, 233, 0.35)' : 'rgba(16, 185, 129, 0.35)';
        ctx.fill();

        // Inner solid point
        ctx.beginPath();
        ctx.arc(hx, hy, 3.5, 0, Math.PI * 2);
        ctx.fillStyle = isCurrentMode ? '#38bdf8' : '#34d399';
        ctx.fill();
        ctx.strokeStyle = '#ffffff';
        ctx.lineWidth = 1.5;
        ctx.stroke();
    }
}

function setupChartInteractions() {
    const canvas = document.getElementById('sessionChartCanvas');
    const tooltip = document.getElementById('sessionChartTooltip');
    if (!canvas) return;

    function handlePointer(clientX, clientY) {
        if (!currentChartPoints || currentChartPoints.length === 0) return;

        const rect = canvas.getBoundingClientRect();
        const mouseX = clientX - rect.left;
        const mouseY = clientY - rect.top;

        const { padLeft, padRight, width } = chartCanvasGeometry;
        const plotW = Math.max(10, width - padLeft - padRight);

        if (mouseX < padLeft - 10 || mouseX > width - padRight + 10) {
            hideTooltip();
            return;
        }

        const maxT = Math.max(currentChartPoints[currentChartPoints.length - 1].t, 60);
        const ratio = Math.max(0, Math.min(1, (mouseX - padLeft) / plotW));
        const targetT = ratio * maxT;

        // Binary search or linear search for closest point
        let closestIdx = 0;
        let minDiff = Infinity;
        for (let i = 0; i < currentChartPoints.length; i++) {
            const diff = Math.abs(currentChartPoints[i].t - targetT);
            if (diff < minDiff) {
                minDiff = diff;
                closestIdx = i;
            }
        }

        const p = currentChartPoints[closestIdx];
        drawChart(currentChartPoints, closestIdx);

        if (tooltip) {
            const timeStr = formatSessionDuration(p.t);
            const stateLabel = p.state ? formatSessionState(p.state) : '';
            const stateStr = stateLabel ? ` • ${stateLabel}` : '';
            if (sessionMetricMode === 'current') {
                tooltip.innerHTML = `
                    <div class="font-bold text-sky-400 font-mono">${p.amps.toFixed(1)} A <span class="text-slate-300 font-normal">(${p.kw.toFixed(2)} kW)</span></div>
                    <div class="text-[9px] text-slate-400 font-mono mt-0.5">${timeStr}${stateStr}</div>
                `;
            } else {
                tooltip.innerHTML = `
                    <div class="font-bold text-emerald-400 font-mono">${p.kw.toFixed(2)} kW <span class="text-slate-300 font-normal">(${p.amps.toFixed(1)} A)</span></div>
                    <div class="text-[9px] text-slate-400 font-mono mt-0.5">${timeStr}${stateStr}</div>
                `;
            }
            tooltip.style.display = 'block';
            tooltip.style.left = `${mouseX}px`;
            tooltip.style.top = `${mouseY}px`;
        }
    }

    function hideTooltip() {
        if (tooltip) tooltip.style.display = 'none';
        drawChart(currentChartPoints, -1);
    }

    canvas.addEventListener('mousemove', (e) => handlePointer(e.clientX, e.clientY));
    canvas.addEventListener('mouseleave', hideTooltip);

    canvas.addEventListener('touchmove', (e) => {
        if (e.touches && e.touches.length > 0) {
            handlePointer(e.touches[0].clientX, e.touches[0].clientY);
        }
    }, { passive: true });
    canvas.addEventListener('touchend', hideTooltip);
}

// ==============================================================================
// DOWNLOAD ZIP ACTION
// ==============================================================================

async function downloadSelectedSessionZip() {
    if (window.isAuthRequired && !window.isAuthenticated) {
        if (typeof showAuthModal === 'function') showAuthModal(false);
        return;
    }
    if (!selectedSessionId) return;

    const actualId = (selectedSessionId === 'active')
        ? (sessionsCache.active_session ? sessionsCache.active_session.session_id : null)
        : selectedSessionId;

    if (!actualId) return;

    const btnZip = document.getElementById('btnSessionDownloadZip');
    const iconZip = document.getElementById('iconSessionDownloadZip');
    const iconSpinner = document.getElementById('iconSessionDownloadSpinner');

    try {
        if (iconZip) iconZip.classList.add('hidden');
        if (iconSpinner) iconSpinner.classList.remove('hidden');
        if (btnZip) btnZip.disabled = true;

        const res = await apiFetch(`/api/sessions/${encodeURIComponent(actualId)}/export?format=zip`);

        if (!res.ok) {
            let errMsg = 'Export failed';
            try {
                const errData = await res.json();
                if (errData && errData.error) errMsg = errData.error;
            } catch (_) {}
            alert('Export error: ' + errMsg);
            return;
        }

        const blob = await res.blob();
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.style.display = 'none';
        a.href = url;
        a.download = `${actualId}.zip`;
        document.body.appendChild(a);
        a.click();
        window.URL.revokeObjectURL(url);
        document.body.removeChild(a);
    } catch (e) {
        console.error('Download error:', e);
        alert('Download error: ' + e);
    } finally {
        if (iconSpinner) iconSpinner.classList.add('hidden');
        if (iconZip) iconZip.classList.remove('hidden');
        if (btnZip) btnZip.disabled = false;
    }
}

// ==============================================================================
// DELETE SESSION ACTION (with safety confirmation)
// ==============================================================================

function resetDeleteButton() {
    if (deleteConfirmTimer) {
        clearTimeout(deleteConfirmTimer);
        deleteConfirmTimer = null;
    }
    const btnDel = document.getElementById('btnSessionDelete');
    const lblDel = document.getElementById('lblSessionDelete');
    if (btnDel) btnDel.classList.remove('confirming');
    if (lblDel) {
        lblDel.innerText = '';
        lblDel.classList.add('hidden');
    }
}

async function handleDeleteSessionClick() {
    if (window.isAuthRequired && !window.isAuthenticated) {
        if (typeof showAuthModal === 'function') showAuthModal(false);
        return;
    }
    if (!selectedSessionId || selectedSessionId === 'active') return;

    const btnDel = document.getElementById('btnSessionDelete');
    const lblDel = document.getElementById('lblSessionDelete');

    if (!btnDel.classList.contains('confirming')) {
        // First click: ask for confirmation
        btnDel.classList.add('confirming');
        if (lblDel) {
            lblDel.innerText = (typeof t === 'function' ? t('sessions.delete_confirm') : 'Confirmer ?') || 'Confirmer ?';
            lblDel.classList.remove('hidden');
        }

        deleteConfirmTimer = setTimeout(() => {
            resetDeleteButton();
        }, 4000);
    } else {
        // Second click: execute deletion
        resetDeleteButton();
        const idToDelete = selectedSessionId;

        try {
            const res = await apiFetch(`/api/sessions/${encodeURIComponent(idToDelete)}`, {
                method: 'DELETE'
            });

            if (!res.ok) {
                const errData = await res.json().catch(() => ({}));
                alert((t('sessions.delete_failed') || 'Échec : ') + (errData.error || res.statusText));
                return;
            }

            // Remove from local cache
            sessionCsvCache.delete(idToDelete);
            selectedSessionId = null;

            // Reload sessions list
            await loadSessionsData(true);
        } catch (e) {
            console.error('Delete error:', e);
            alert('Delete error: ' + e);
        }
    }
}

// ==============================================================================
// INITIALIZATION
// ==============================================================================

let sessionsDomListenersAttached = false;

function initSessions() {
    if (!sessionsDomListenersAttached) {
        sessionsDomListenersAttached = true;
        // Navigation Tabs Listeners
        const tabBoard = document.getElementById('tabBoard');
        const tabSessions = document.getElementById('tabSessions');
        const tabConsole = document.getElementById('tabConsole');
        const tabSystem = document.getElementById('tabSystem') || document.getElementById('tabConfig');

        if (tabBoard) tabBoard.addEventListener('click', () => switchMainTab('board'));
        if (tabSessions) tabSessions.addEventListener('click', () => switchMainTab('sessions'));
        if (tabConsole) tabConsole.addEventListener('click', () => switchMainTab('console'));
        if (tabSystem) tabSystem.addEventListener('click', () => switchMainTab('system'));

        // Dropdown change listener
        const select = document.getElementById('sessionSelect');
        if (select) {
            select.addEventListener('change', (e) => {
                onSessionSelected(e.target.value);
            });
        }

        // Action buttons listeners
        const btnZip = document.getElementById('btnSessionDownloadZip');
        if (btnZip) btnZip.addEventListener('click', downloadSelectedSessionZip);

        const btnDel = document.getElementById('btnSessionDelete');
        if (btnDel) btnDel.addEventListener('click', handleDeleteSessionClick);

        // Metric unit toggles (kW / A)
        const btnToggleMetric = document.getElementById('btnSessionMetricToggle');
        if (btnToggleMetric) {
            btnToggleMetric.addEventListener('click', () => {
                const nextMode = (sessionMetricMode === 'power') ? 'current' : 'power';
                setSessionMetricMode(nextMode);
            });
        }

        const btnChartKw = document.getElementById('btnChartModeKw');
        if (btnChartKw) {
            btnChartKw.addEventListener('click', () => {
                setSessionMetricMode('power');
            });
        }

        const btnChartAmps = document.getElementById('btnChartModeAmps');
        if (btnChartAmps) {
            btnChartAmps.addEventListener('click', () => {
                setSessionMetricMode('current');
            });
        }

        // Chart interactions & resize
        setupChartInteractions();
        window.addEventListener('resize', () => {
            const sessionsPanel = document.getElementById('sessionsPanel');
            if (sessionsPanel && !sessionsPanel.classList.contains('hidden')) {
                resizeAndRedrawChart();
            }
        });
    }

    // Apply stored metric mode (power / current)
    setSessionMetricMode(sessionMetricMode);

    // Check stored active tab
    const savedTab = localStorage.getItem('greenup_active_tab') || 'board';
    switchMainTab(savedTab);
}

// Expose updateSessionsDropdown and setSessionMetricMode to global scope
window.updateSessionsDropdown = updateSessionsDropdown;
window.setSessionMetricMode = setSessionMetricMode;
