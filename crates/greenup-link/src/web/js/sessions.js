// Sessions Management & Power Curve Visualization module
// Handles tab switching, session listing, metrics display, CSV chart rendering, ZIP export, and deletion.

let sessionsCache = { active_session: null, sessions: [] };
let selectedSessionId = null;
let sessionCsvCache = new Map();
let currentChartPoints = [];
let deleteConfirmTimer = null;

function formatSessionDuration(totalSeconds) {
    if (totalSeconds == null || isNaN(totalSeconds)) return '--';
    const s = Math.max(0, Math.floor(totalSeconds));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    const hLabel = t('sessions.time_h') || 'h';
    const mLabel = t('sessions.time_m') || 'm';
    const sLabel = t('sessions.time_s') || 's';

    if (h > 0) {
        return `${h}${hLabel} ${m.toString().padStart(2, '0')}${mLabel}`;
    }
    if (m > 0) {
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
    if (!window.isAuthResolved || (window.isAuthRequired && !window.isAuthenticated)) return;

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
        opt.innerText = `${prefix} ${act.start_time} — ${act.total_energy_kwh.toFixed(2)} kWh (${formatSessionDuration(act.charging_duration_seconds)})`;
        select.appendChild(opt);
    }

    // 2. Saved sessions list
    savedList.forEach(s => {
        const opt = document.createElement('option');
        opt.value = s.id;
        const sideBadge = (window.isDualStation && s.side) ? `[P${s.side}] ` : '';
        opt.innerText = `📅 ${sideBadge}${s.start_time} — ${s.total_energy_kwh.toFixed(2)} kWh (${formatSessionDuration(s.charging_duration_seconds)})`;
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
    setElemText('sessionValMaxPower', '--');
    setElemText('sessionValAvgPower', '--');
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
        setElemText('sessionValPhase', act.phase || 'Mono');
        setElemText('sessionValMaxPower', `${(act.max_power_w / 1000).toFixed(2)} kW`);
        setElemText('sessionValAvgPower', `Max: ${act.max_current_a.toFixed(1)} A`);
        const reasonEl = document.getElementById('sessionValReason');
        if (reasonEl) {
            reasonEl.removeAttribute('data-i18n');
            reasonEl.innerText = `${t('sessions.active_reason') || 'En cours'} (État ${act.state})`;
            reasonEl.title = '';
        }
        setElemText('sessionValEndTime', `Inst: ${(act.power_w / 1000).toFixed(2)} kW (${act.current_a.toFixed(1)} A)`);

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
        setElemText('sessionValPhase', s.phase || 'Mono');
        setElemText('sessionValMaxPower', `${(s.max_power_w / 1000).toFixed(2)} kW`);
        setElemText('sessionValAvgPower', `Moy: ${(s.avg_power_w / 1000).toFixed(2)} kW`);
        const reasonElem = document.getElementById('sessionValReason');
        if (reasonElem) {
            reasonElem.removeAttribute('data-i18n');
            reasonElem.innerText = s.termination_reason || '--';
            reasonElem.title = s.termination_reason || '';
        }
        setElemText('sessionValEndTime', s.end_time || '--');

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
    if (statusElem) statusElem.innerText = t('sessions.chart_loading') || 'Chargement...';

    if (sessionCsvCache.has(sessionId)) {
        const cachedPoints = sessionCsvCache.get(sessionId);
        currentChartPoints = cachedPoints;
        drawChart(cachedPoints);
        if (statusElem) statusElem.innerText = `${cachedPoints.length} pts`;
        return;
    }

    try {
        const res = await apiFetch(`/api/sessions/${encodeURIComponent(sessionId)}/export`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ format: 'csv' })
        });

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

    const headers = lines[0].split(',').map(h => h.trim());
    const elapsedIdx = headers.indexOf('Elapsed_s');
    const powerIdx = headers.indexOf('Power_W');
    const currentIdx = headers.indexOf('Current_A');
    const stateIdx = headers.indexOf('State');

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
    let maxKw = 0;
    for (let p of points) {
        if (p.kw > maxKw) maxKw = p.kw;
    }
    // Round maxKw up to next pleasant step (e.g. 4.0, 7.5, 11.0, 22.0)
    let yMax = Math.ceil(Math.max(maxKw * 1.15, 3.7));

    // Coordinates mapping
    const getX = (t) => padLeft + (t / maxT) * plotW;
    const getY = (kw) => padTop + plotH - (kw / yMax) * plotH;

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
        ctx.fillText(`${stepVal.toFixed(1)}k`, padLeft - 6, y);
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
    gradient.addColorStop(0, 'rgba(16, 185, 129, 0.35)');
    gradient.addColorStop(0.7, 'rgba(14, 165, 233, 0.15)');
    gradient.addColorStop(1, 'rgba(15, 23, 42, 0.0)');

    ctx.beginPath();
    ctx.moveTo(getX(points[0].t), getY(0));
    for (let i = 0; i < points.length; i++) {
        ctx.lineTo(getX(points[i].t), getY(points[i].kw));
    }
    ctx.lineTo(getX(points[points.length - 1].t), getY(0));
    ctx.closePath();
    ctx.fillStyle = gradient;
    ctx.fill();

    // 4. Draw curve line
    ctx.beginPath();
    ctx.strokeStyle = '#10b981';
    ctx.lineWidth = 2;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    for (let i = 0; i < points.length; i++) {
        const x = getX(points[i].t);
        const y = getY(points[i].kw);
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
    }
    ctx.stroke();

    // 5. Draw hover marker and crosshair if requested
    if (hoverIndex >= 0 && hoverIndex < points.length) {
        const p = points[hoverIndex];
        const hx = getX(p.t);
        const hy = getY(p.kw);

        // Vertical dashed line
        ctx.setLineDash([3, 3]);
        ctx.strokeStyle = 'rgba(56, 189, 248, 0.8)';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.moveTo(hx, padTop);
        ctx.lineTo(hx, padTop + plotH);
        ctx.stroke();
        ctx.setLineDash([]);

        // Outer glow
        ctx.beginPath();
        ctx.arc(hx, hy, 6, 0, Math.PI * 2);
        ctx.fillStyle = 'rgba(56, 189, 248, 0.35)';
        ctx.fill();

        // Inner solid point
        ctx.beginPath();
        ctx.arc(hx, hy, 3.5, 0, Math.PI * 2);
        ctx.fillStyle = '#38bdf8';
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
            const stateStr = p.state ? ` • État ${p.state}` : '';
            tooltip.innerHTML = `
                <div class="font-bold text-sky-400 font-mono">${p.kw.toFixed(2)} kW <span class="text-slate-300 font-normal">(${p.amps.toFixed(1)} A)</span></div>
                <div class="text-[9px] text-slate-400 font-mono mt-0.5">${timeStr}${stateStr}</div>
            `;
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

        const res = await apiFetch(`/api/sessions/${encodeURIComponent(actualId)}/export`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ format: 'zip' })
        });

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

        // Chart interactions & resize
        setupChartInteractions();
        window.addEventListener('resize', () => {
            const sessionsPanel = document.getElementById('sessionsPanel');
            if (sessionsPanel && !sessionsPanel.classList.contains('hidden')) {
                resizeAndRedrawChart();
            }
        });
    }

    // Check stored active tab
    const savedTab = localStorage.getItem('greenup_active_tab') || 'board';
    switchMainTab(savedTab);
}

// Expose updateSessionsDropdown to global scope so i18n can trigger re-renders on language changes
window.updateSessionsDropdown = updateSessionsDropdown;
