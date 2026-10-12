// Charging visualizer: charger box, cables, car, state label and action buttons appearance.
// The charging cable animation runs at a FIXED speed (defined in css/style.css, .anim-flow).
// Variable speed (proportional to current) was removed: restarting the CSS animation on every
// current change caused visual glitches.

function updateVisuals(greenup_state, isChargeComplete, iec_state, error_code, charge_paused, charge_authorized) {
    if (greenup_state === window.lastVisualState &&
        iec_state === window.lastIecState &&
        isChargeComplete === window.lastChargeComplete &&
        isActionPending === window.lastActionPending &&
        charge_paused === window.lastChargePaused &&
        charge_authorized === window.lastChargeAuthorized) {
        return;
    }
    window.lastActionPending = isActionPending;

    window.lastVisualState = greenup_state;
    window.lastIecState = iec_state;
    window.lastChargeComplete = isChargeComplete;
    window.lastChargePaused = charge_paused;
    window.lastChargeAuthorized = charge_authorized;

    // Reset base styles
    chargerBox.className = 'w-16 h-24 md:w-20 md:h-28 rounded-2xl bg-slate-950 border-2 border-slate-800 flex flex-col items-center justify-center relative z-10 transition-all duration-500 shadow-2xl';
    chargerIcon.className = 'w-8 h-8 md:w-10 md:h-10 text-slate-600 transition-colors duration-500';
    chargerLed.className = 'w-6 h-1 md:w-8 md:h-1.5 rounded-full bg-slate-700 mt-4 transition-all duration-500';
    carIcon.className = 'w-full h-full text-slate-700 transition-all duration-500 drop-shadow-2xl';
    carBox.classList.remove('scale-105');

    const animWire1 = document.getElementById('animWire1');
    const animWire2 = document.getElementById('animWire2');
    if (animWire1) { animWire1.classList.add('opacity-0'); animWire1.classList.remove('anim-flow'); animWire1.setAttribute('stroke', '#10b981'); }
    if (animWire2) { animWire2.classList.add('opacity-0'); animWire2.classList.remove('anim-flow'); animWire2.setAttribute('stroke', '#10b981'); }

    carIcon.style.fill = '';
    carIcon.classList.remove('animate-pulse');
    chargerBox.classList.remove('animate-pulse');

    let stateLabel = iec_state || greenup_state;
    switch(iec_state) {
        case 'A':
            stateLabel = t('state.disconnected') + " (" + greenup_state + ")";
            break;

        case 'B':
            stateLabel = t('state.connected') + " (" + greenup_state + ")";
            break;

        case 'C':
            stateLabel = t('state.charging') + " (" + greenup_state + ")";

            // Animated green cable (fixed speed)
            if(animWire1) {
                animWire1.classList.remove('opacity-0');
                animWire1.classList.add('anim-flow');
            }
            if(animWire2) {
                animWire2.classList.remove('opacity-0');
                animWire2.classList.add('anim-flow');
            }
            // Green charger
            chargerBox.classList.add('border-emerald-500/50', 'shadow-[0_0_30px_rgba(16,185,129,0.15)]');
            chargerIcon.classList.replace('text-slate-600', 'text-emerald-400');
            chargerLed.classList.replace('bg-slate-700', 'bg-emerald-400');
            chargerLed.classList.add('shadow-[0_0_10px_rgba(16,185,129,0.8)]');

            // Animated green car
            carBox.classList.add('scale-105');
            carIcon.style.fill = 'url(#waveGradient)';
            break;

        case 'E':
        case 'F':
            stateLabel = `${t('state.fault')} (${iec_state} / ${greenup_state}) - ${t('state.code')}: ${error_code || t('state.unknown')}`;
            // Red charger, cable and car
            chargerBox.classList.add('border-rose-500/50', 'shadow-[0_0_30px_rgba(244,63,94,0.15)]');
            chargerIcon.classList.replace('text-slate-600', 'text-rose-500');
            chargerLed.classList.replace('bg-slate-700', 'bg-rose-500');

            carIcon.classList.replace('text-slate-700', 'text-rose-500');

            if(animWire1) { animWire1.classList.remove('opacity-0'); animWire1.setAttribute('stroke', '#f43f5e'); }
            if(animWire2) { animWire2.classList.remove('opacity-0'); animWire2.setAttribute('stroke', '#f43f5e'); }
            break;

        default:
            stateLabel = iec_state || greenup_state;
            break;
    }

    // Special case: charge physically completed (e.g. L or M after charging)
    if (isChargeComplete && (iec_state === 'A' || iec_state === 'B')) {
        stateLabel = t('state.charge_complete') + " (" + greenup_state + ")";
        carIcon.classList.replace('text-slate-700', 'text-emerald-500');
        carIcon.classList.replace('text-sky-500', 'text-emerald-500');
    }

    updateControlButtons(charge_authorized, charge_paused);

    lblState.innerText = stateLabel;
}
