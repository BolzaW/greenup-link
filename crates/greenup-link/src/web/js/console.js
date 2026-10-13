// Live Logs & Raw Command Console module
// Connects to SSE (/api/logs/stream) or falls back to polling /api/logs.
// Handles real-time rendering, filtering by type/level, search, and raw command prompt.

let logEntries = [];
let maxLogEntries = 500;
let isLogStreamPaused = false;
let userScrolledUp = false;
let currentTypeFilter = 'ALL';
let currentLevelFilter = 'DEBUG';
let currentSearchTerm = '';
let commandHistory = [];
let commandHistoryIndex = -1;
let logEventSource = null;

const LOG_LEVEL_SEVERITY = {
    'DEBUG': 1,
    'INFO': 2,
    'WARN': 3,
    'ERROR': 4
};

function initConsole() {
    setupConsoleDomListeners();
    loadConsoleHistory();
    const debugStored = localStorage.getItem('greenup_debug_mode');
    if (debugStored === 'true') {
        setDebugMode(true);
    }
}

function setDebugMode(enabled) {
    const visualizerPanel = document.getElementById('visualizerPanel');
    const consolePanel = document.getElementById('consolePanel');
    const toggleDebug = document.getElementById('toggleDebug');

    if (toggleDebug) toggleDebug.checked = enabled;
    localStorage.setItem('greenup_debug_mode', enabled ? 'true' : 'false');

    if (enabled) {
        if (visualizerPanel) visualizerPanel.classList.add('hidden');
        if (consolePanel) consolePanel.classList.remove('hidden');
        startLogStream();
    } else {
        if (consolePanel) consolePanel.classList.add('hidden');
        if (visualizerPanel) visualizerPanel.classList.remove('hidden');
        stopLogStream();
    }
}

function startLogStream() {
    if (logEventSource) return;

    fetch('/api/logs')
        .then(res => res.json())
        .then(logs => {
            if (Array.isArray(logs)) {
                logEntries = logs;
                renderLogs();
            }
            connectSse();
        })
        .catch(err => {
            console.warn('Could not fetch /api/logs:', err);
            connectSse();
        });
}

function connectSse() {
    try {
        logEventSource = new EventSource('/api/logs/stream');
        logEventSource.onmessage = function(event) {
            if (isLogStreamPaused) return;
            try {
                const entry = JSON.parse(event.data);
                addLogEntry(entry);
            } catch (e) {
                console.error('Error parsing log entry SSE:', e);
            }
        };
        logEventSource.onerror = function() {
            // EventSource automatically retries
        };
    } catch (e) {
        console.warn('SSE not supported or connection error:', e);
    }
}

function stopLogStream() {
    if (logEventSource) {
        logEventSource.close();
        logEventSource = null;
    }
}

function addLogEntry(entry) {
    logEntries.push(entry);
    if (logEntries.length > maxLogEntries) {
        logEntries.shift();
    }

    if (matchesFilter(entry)) {
        appendLogDom(entry);
        if (!isLogStreamPaused && !userScrolledUp) {
            scrollConsoleToBottom();
        }
    }
    updateLogCount();
}

function matchesFilter(entry) {
    if (currentTypeFilter !== 'ALL') {
        if (currentTypeFilter === 'SERIAL' && !entry.category.startsWith('SERIAL')) return false;
        if (currentTypeFilter === 'API' && entry.category !== 'API') return false;
        if (currentTypeFilter === 'SYS' && entry.category !== 'SYS' && entry.category !== 'IHM') return false;
    }
    if (currentLevelFilter) {
        const minSeverity = LOG_LEVEL_SEVERITY[currentLevelFilter] || 1;
        const entrySeverity = LOG_LEVEL_SEVERITY[entry.level] || 2;
        if (entrySeverity < minSeverity) return false;
    }
    if (currentSearchTerm) {
        const full = (entry.timestamp + " " + entry.category + " " + entry.message).toLowerCase();
        if (!full.includes(currentSearchTerm)) return false;
    }
    return true;
}

function renderLogs() {
    const list = document.getElementById('consoleLogList');
    if (!list) return;
    list.innerHTML = '';
    const filtered = logEntries.filter(matchesFilter);
    const fragment = document.createDocumentFragment();
    for (const entry of filtered) {
        fragment.appendChild(createLogLineElement(entry));
    }
    list.appendChild(fragment);
    if (!userScrolledUp) {
        scrollConsoleToBottom();
    }
    updateLogCount();
}

function createLogLineElement(entry) {
    const line = document.createElement('div');
    line.className = 'console-line';

    // Time
    const timeSpan = document.createElement('span');
    timeSpan.className = 'console-time';
    timeSpan.textContent = entry.timestamp;

    // Badge category
    const catSpan = document.createElement('span');
    catSpan.className = getCategoryBadgeClass(entry.category);
    catSpan.textContent = `[${entry.category}]`;

    // Message
    const msgSpan = document.createElement('span');
    msgSpan.className = getMessageClass(entry);
    msgSpan.textContent = entry.message;

    line.appendChild(timeSpan);
    line.appendChild(catSpan);
    line.appendChild(msgSpan);
    return line;
}

function appendLogDom(entry) {
    const list = document.getElementById('consoleLogList');
    if (!list) return;
    list.appendChild(createLogLineElement(entry));
    if (list.children.length > maxLogEntries) {
        list.removeChild(list.firstChild);
    }
}

function getCategoryBadgeClass(cat) {
    if (cat === 'SERIAL_RX') return 'console-badge-rx';
    if (cat === 'SERIAL_TX') return 'console-badge-tx';
    if (cat === 'API') return 'console-badge-api';
    if (cat === 'DEBUG' || cat === 'TRACE') return 'console-badge-dbg';
    if (cat === 'WARN') return 'console-badge-warn';
    if (cat === 'ERROR') return 'console-badge-err';
    return 'console-badge-sys';
}

function getMessageClass(entry) {
    if (entry.level === 'ERROR') return 'console-msg-err';
    if (entry.level === 'WARN') return 'console-msg-warn';
    if (entry.category === 'SERIAL_RX') return 'console-msg-rx';
    if (entry.category === 'SERIAL_TX') return 'console-msg-tx';
    if (entry.category === 'API') return 'console-msg-api';
    if (entry.level === 'DEBUG') return 'console-msg-dbg';
    return 'console-msg';
}

function scrollConsoleToBottom() {
    const container = document.getElementById('consoleScrollArea');
    if (container) {
        container.scrollTop = container.scrollHeight;
    }
}

function updateLogCount() {
    const countBadge = document.getElementById('consoleLogCount');
    if (countBadge) {
        countBadge.textContent = `${logEntries.length} logs`;
    }
}

function setupConsoleDomListeners() {
    // Toggle debug header switch
    const toggleDebug = document.getElementById('toggleDebug');
    if (toggleDebug) {
        toggleDebug.addEventListener('change', (e) => {
            setDebugMode(e.target.checked);
        });
    }

    // Scroll listener for detecting user scrolling up
    const scrollArea = document.getElementById('consoleScrollArea');
    if (scrollArea) {
        scrollArea.addEventListener('scroll', () => {
            const distanceFromBottom = scrollArea.scrollHeight - scrollArea.scrollTop - scrollArea.clientHeight;
            userScrolledUp = distanceFromBottom > 35;
        });
    }

    // Type filters
    const typePills = document.querySelectorAll('[data-log-type]');
    typePills.forEach(pill => {
        pill.addEventListener('click', () => {
            typePills.forEach(p => p.classList.remove('active'));
            pill.classList.add('active');
            currentTypeFilter = pill.getAttribute('data-log-type');
            renderLogs();
        });
    });

    // Level filters
    const levelPills = document.querySelectorAll('[data-log-level]');
    levelPills.forEach(pill => {
        pill.addEventListener('click', () => {
            levelPills.forEach(p => p.classList.remove('active'));
            pill.classList.add('active');
            currentLevelFilter = pill.getAttribute('data-log-level');
            renderLogs();
        });
    });

    // Search input
    const searchInput = document.getElementById('consoleSearchInput');
    if (searchInput) {
        searchInput.addEventListener('input', (e) => {
            currentSearchTerm = e.target.value.trim().toLowerCase();
            renderLogs();
        });
    }

    // Copy visible logs button
    const btnCopy = document.getElementById('btnConsoleCopy');
    if (btnCopy) {
        btnCopy.addEventListener('click', copyVisibleLogs);
    }

    // Export active log file button
    const btnExport = document.getElementById('btnConsoleExport');
    if (btnExport) {
        btnExport.addEventListener('click', () => {
            window.location.href = '/api/logs/export';
        });
    }

    // Clear logs button
    const btnClear = document.getElementById('btnConsoleClear');
    if (btnClear) {
        btnClear.addEventListener('click', () => {
            logEntries = [];
            renderLogs();
        });
    }

    // Pause/Resume autoscroll button
    const btnPause = document.getElementById('btnConsolePause');
    if (btnPause) {
        btnPause.addEventListener('click', () => {
            isLogStreamPaused = !isLogStreamPaused;
            btnPause.classList.toggle('active', isLogStreamPaused);
            const iconPause = document.getElementById('consoleIconPause');
            const iconPlay = document.getElementById('consoleIconPlay');
            if (iconPause && iconPlay) {
                iconPause.classList.toggle('hidden', isLogStreamPaused);
                iconPlay.classList.toggle('hidden', !isLogStreamPaused);
            }
            if (!isLogStreamPaused) {
                userScrolledUp = false;
                scrollConsoleToBottom();
            }
        });
    }

    // Raw prompt send
    const btnSend = document.getElementById('btnConsoleSend');
    const inputPrompt = document.getElementById('consoleCommandInput');
    if (btnSend && inputPrompt) {
        const sendCmd = () => {
            const raw = inputPrompt.value.trim();
            if (!raw) return;
            sendCommandRaw(raw);
            addCommandToHistory(raw);
            inputPrompt.value = '';
        };

        btnSend.addEventListener('click', sendCmd);
        inputPrompt.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                e.preventDefault();
                sendCmd();
            } else if (e.key === 'ArrowUp') {
                e.preventDefault();
                navigateHistory(-1, inputPrompt);
            } else if (e.key === 'ArrowDown') {
                e.preventDefault();
                navigateHistory(1, inputPrompt);
            }
        });
    }
}

function sendCommandRaw(cmd) {
    const feedback = document.getElementById('consoleCommandFeedback');
    if (feedback) {
        feedback.textContent = `Envoi: ${cmd}...`;
        feedback.className = 'text-[10px] font-mono text-sky-400';
    }

    fetch('/api/command', {
        method: 'POST',
        headers: { 'Content-Type': 'text/plain' },
        body: cmd
    })
    .then(async res => {
        const data = await res.json().catch(() => ({}));
        if (res.ok) {
            if (feedback) {
                feedback.textContent = `OK: ${data.message || 'Commande envoyée'}`;
                feedback.className = 'text-[10px] font-mono text-emerald-400';
                setTimeout(() => { if (feedback.textContent.startsWith('OK:')) feedback.textContent = ''; }, 3000);
            }
        } else {
            if (feedback) {
                feedback.textContent = `Erreur: ${data.error || res.statusText}`;
                feedback.className = 'text-[10px] font-mono text-rose-400';
            }
        }
    })
    .catch(err => {
        if (feedback) {
            feedback.textContent = `Erreur réseau: ${err.message}`;
            feedback.className = 'text-[10px] font-mono text-rose-400';
        }
    });
}

function loadConsoleHistory() {
    try {
        const saved = localStorage.getItem('greenup_cmd_history');
        if (saved) commandHistory = JSON.parse(saved);
    } catch (_) {}
}

function addCommandToHistory(cmd) {
    if (commandHistory[commandHistory.length - 1] !== cmd) {
        commandHistory.push(cmd);
        if (commandHistory.length > 50) commandHistory.shift();
        localStorage.setItem('greenup_cmd_history', JSON.stringify(commandHistory));
    }
    commandHistoryIndex = commandHistory.length;
}

function navigateHistory(direction, input) {
    if (commandHistory.length === 0) return;
    commandHistoryIndex += direction;
    if (commandHistoryIndex < 0) commandHistoryIndex = 0;
    if (commandHistoryIndex >= commandHistory.length) {
        commandHistoryIndex = commandHistory.length;
        input.value = '';
        return;
    }
    input.value = commandHistory[commandHistoryIndex];
}

function copyVisibleLogs() {
    const visibleEntries = logEntries.filter(matchesFilter);
    if (visibleEntries.length === 0) {
        showCopyFeedback(false);
        return;
    }
    const textToCopy = visibleEntries.map(e => `[${e.timestamp}] [${e.level}] [${e.category}] ${e.message}`).join('\n');

    if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(textToCopy)
            .then(() => showCopyFeedback(true))
            .catch(() => fallbackCopy(textToCopy));
    } else {
        fallbackCopy(textToCopy);
    }
}

function fallbackCopy(text) {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.focus();
    ta.select();
    try {
        const ok = document.execCommand('copy');
        showCopyFeedback(ok);
    } catch (e) {
        console.error('Failed to copy', e);
        showCopyFeedback(false);
    }
    document.body.removeChild(ta);
}

function showCopyFeedback(success) {
    const iconCopy = document.getElementById('iconCopy');
    const iconCheck = document.getElementById('iconCopyCheck');
    const btn = document.getElementById('btnConsoleCopy');
    if (!iconCopy || !iconCheck || !btn) return;

    if (success) {
        iconCopy.classList.add('hidden');
        iconCheck.classList.remove('hidden');
        btn.classList.add('border-emerald-500/50');
        setTimeout(() => {
            iconCopy.classList.remove('hidden');
            iconCheck.classList.add('hidden');
            btn.classList.remove('border-emerald-500/50');
        }, 1500);
    }
}
