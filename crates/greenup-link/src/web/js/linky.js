function applyTicCapabilities(hasTic) {
    window.stationHasTic = hasTic;

    // 1. Visualizer adaptation (single dynamic layer)
    const grid = document.getElementById('visualizerGrid');
    const staticWire1 = document.getElementById('staticWire1');
    const animWire1 = document.getElementById('animWire1');

    if (grid) {
        if (!hasTic) {
            grid.classList.add('no-tic');
            // Straight run across 6 columns (col 4-9), then arc across 3 columns (col 10-12) to meet bottom loop
            const pathNoTic = "M -5 45 L 67 45 C 88 45 100 65 100 100";
            if (staticWire1 && staticWire1.getAttribute('d') !== pathNoTic) {
                staticWire1.setAttribute('d', pathNoTic);
            }
            if (animWire1 && animWire1.getAttribute('d') !== pathNoTic) {
                animWire1.setAttribute('d', pathNoTic);
            }
        } else {
            grid.classList.remove('no-tic');
            const pathTic = "M -15 45 C 80 45 100 45 100 100";
            if (staticWire1 && staticWire1.getAttribute('d') !== pathTic) {
                staticWire1.setAttribute('d', pathTic);
            }
            if (animWire1 && animWire1.getAttribute('d') !== pathTic) {
                animWire1.setAttribute('d', pathTic);
            }
        }
    }

    // 2. System / Config panel controls adaptation
    const btnRefreshTic = document.getElementById('btnRefreshTicConfig');
    const speedBadge = document.getElementById('cfgTicSpeedBadge');

    if (!hasTic) {
        if (btnRefreshTic) {
            btnRefreshTic.disabled = true;
            btnRefreshTic.title = typeof t === 'function' ? t('system.tic_not_supported') : 'Non supporté sur ce modèle';
        }
        if (speedBadge) {
            speedBadge.className = 'sys-badge sys-badge-muted';
            speedBadge.innerText = typeof t === 'function' ? t('system.tic_unsupported_badge') : 'Non supporté';
        }
    } else {
        if (btnRefreshTic) {
            btnRefreshTic.disabled = false;
            btnRefreshTic.title = '';
        }
        updateTicSpeedBadge(lastReportedTicMode);
    }
}

// Linky (TIC) widget rendering. Two animation speeds: Standard (9600) and Historique (1200).

function updateLinky(ticMode) {
    lastReportedTicMode = ticMode;
    if (window.stationHasTic === false) return;
    updateTicSpeedBadge(ticMode);
    linkyBox.classList.remove('animate-pulse');

    linkyIcon.setAttribute('class', 'w-6 h-6 md:w-8 md:h-8 text-slate-600 transition-colors');
    linkyContainer.className = 'w-14 h-14 md:w-16 md:h-16 rounded-xl bg-slate-950 border-2 border-slate-800 flex items-center justify-center transition-all duration-300 shadow-inner';

    let labelText = "";

    switch(ticMode) {
        case '9600':
            linkyIcon.classList.replace('text-slate-600', 'text-sky-400');
            linkyContainer.classList.add('border-sky-500/30');
            animWireLinky.classList.remove('opacity-0');
            animWireLinky.style.animationDuration = '0.5s';
            labelText = t('tic.standard');
            break;
        case '1200':
            linkyIcon.classList.replace('text-slate-600', 'text-sky-400');
            linkyContainer.classList.add('border-sky-500/30');
            animWireLinky.classList.remove('opacity-0');
            animWireLinky.style.animationDuration = '2.0s';
            labelText = t('tic.historic');
            break;
        case 'detecting':
            linkyBox.classList.add('animate-pulse');
            linkyIcon.classList.replace('text-slate-600', 'text-sky-400');
            linkyContainer.classList.add('border-sky-500/50');
            animWireLinky.classList.add('opacity-0');
            labelText = t('tic.detecting');
            break;
        case '0':
            animWireLinky.classList.add('opacity-0');
            labelText = t('tic.disconnected');
            break;
        default:
            animWireLinky.classList.add('opacity-0');
            labelText = '--';
            break;
    }

    if (labelText !== '--') {
        linkyLabel.classList.remove('opacity-0');
        linkyLabel.innerText = labelText;
    } else {
        linkyLabel.classList.add('opacity-0');
    }
}

function updateTicSpeedBadge(ticMode) {
    if (window.stationHasTic === false) return;
    const badge = document.getElementById('cfgTicSpeedBadge');
    if (!badge) return;

    switch (ticMode) {
        case '9600':
            badge.className = 'sys-badge sys-badge-sky';
            badge.innerText = 'Standard • 9600';
            break;
        case '1200':
            badge.className = 'sys-badge sys-badge-sky';
            badge.innerText = 'Historique • 1200';
            break;
        case 'detecting':
            badge.className = 'sys-badge sys-badge-amber animate-pulse';
            badge.innerText = (typeof t === 'function' ? t('tic.detecting') : 'Détection...') || 'Détection...';
            break;
        case '0':
            badge.className = 'sys-badge sys-badge-muted';
            badge.innerText = (typeof t === 'function' ? t('tic.disconnected') : 'Non connecté') || 'Non connecté';
            break;
        default:
            badge.className = 'sys-badge sys-badge-muted';
            badge.innerText = '--';
            break;
    }
}
