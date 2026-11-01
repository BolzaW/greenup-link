// Linky (TIC) widget rendering. Two animation speeds: Standard (9600) and Historique (1200).

function updateLinky(ticMode) {
    lastReportedTicMode = ticMode;
    updateTicSpeedBadge(ticMode);
    linkyBox.classList.remove('animate-pulse');
    linkyLed.className = 'absolute -top-1 -right-1 w-3 h-3 md:w-4 md:h-4 rounded-full border-2 border-slate-900 bg-slate-700 transition-colors';
    linkyIcon.className = 'w-6 h-6 md:w-8 md:h-8 text-slate-600 transition-colors';
    linkyContainer.className = 'w-14 h-14 md:w-16 md:h-16 rounded-xl bg-slate-950 border-2 border-slate-800 flex items-center justify-center transition-all duration-300 shadow-inner';

    let labelText = "";

    switch(ticMode) {
        case '9600':
            linkyLed.classList.replace('bg-slate-700', 'bg-sky-500');
            linkyLed.classList.add('shadow-[0_0_8px_rgba(14,165,233,0.8)]');
            linkyIcon.classList.replace('text-slate-600', 'text-sky-400');
            linkyContainer.classList.add('border-sky-500/30');
            animWireLinky.classList.remove('opacity-0');
            animWireLinky.style.animationDuration = '0.5s';
            labelText = t('tic.standard');
            break;
        case '1200':
            linkyLed.classList.replace('bg-slate-700', 'bg-sky-500');
            linkyLed.classList.add('shadow-[0_0_8px_rgba(14,165,233,0.8)]');
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
    const badge = document.getElementById('cfgTicSpeedBadge');
    if (!badge) return;

    switch (ticMode) {
        case '9600':
            badge.className = 'px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-sky-500/20 text-sky-400 border border-sky-500/40';
            badge.innerText = 'Standard (9600 bauds)';
            break;
        case '1200':
            badge.className = 'px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-sky-500/20 text-sky-400 border border-sky-500/40';
            badge.innerText = 'Historique (1200 bauds)';
            break;
        case 'detecting':
            badge.className = 'px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/20 text-amber-400 border border-amber-500/40 animate-pulse';
            badge.innerText = (typeof t === 'function' ? t('tic.detecting') : 'Détection...') || 'Détection...';
            break;
        case '0':
            badge.className = 'px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-slate-800 text-slate-400 border border-slate-700';
            badge.innerText = (typeof t === 'function' ? t('tic.disconnected') : 'Non connecté') || 'Non connecté';
            break;
        default:
            badge.className = 'px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-slate-800 text-slate-500 border border-slate-700';
            badge.innerText = '--';
            break;
    }
}
