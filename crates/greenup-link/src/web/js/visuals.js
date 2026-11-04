// Charging visualizer: charger box, 3-LED indicator bar, cables, car, state label and action buttons appearance.
// Accurately reflects Legrand firmware states (A, B, C, D, E, F, G, H, L, R, V, X...):
// - 3 LEDs bar: independent hardware indicator simulation
//   * State A & idle: 3 static white LEDs (matching Green'Up brand text)
//   * State B, C, F: White LED sequential chase (2s cycle: 500ms all off, 500ms led1, 500ms led1+2, 500ms all on)
//   * State D, E (Type 2 charging) & G, H (Schuko charging): Green LED sequential chase with same pattern
//   * Fault states: 3 solid red LEDs
// - State B (Type 2) & F (Schuko): Charger box pulses slowly in blue (waiting for station authorization)
// - State C (Type 2): Charger box solid green, Car pulses slowly in blue (waiting for vehicle request)
// - State D/E (Type 2) & G/H (Schuko): Active charging with animated green wave car, flowing cable, subtle green breathing pulse on charger box, and electric blue lightning bolt
// - Fault states: Red warning styling on charger box and car

function updateChargerLeds(mode) {
    if (!chargerLed1 || !chargerLed2 || !chargerLed3) return;
    switch (mode) {
        case 'chase-white':
            chargerLed1.className = 'charger-led led-white led-chase-1';
            chargerLed2.className = 'charger-led led-white led-chase-2';
            chargerLed3.className = 'charger-led led-white led-chase-3';
            break;
        case 'chase-green':
            chargerLed1.className = 'charger-led led-green led-chase-1';
            chargerLed2.className = 'charger-led led-green led-chase-2';
            chargerLed3.className = 'charger-led led-green led-chase-3';
            break;
        case 'static-red':
            chargerLed1.className = 'charger-led led-red';
            chargerLed2.className = 'charger-led led-red';
            chargerLed3.className = 'charger-led led-red';
            break;
        case 'static-white':
        default:
            chargerLed1.className = 'charger-led led-white';
            chargerLed2.className = 'charger-led led-white';
            chargerLed3.className = 'charger-led led-white';
            break;
    }
}

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

    // Reset base styles (use setAttribute for SVG elements whose className is SVGAnimatedString)
    chargerBox.className = 'w-16 h-24 md:w-20 md:h-28 rounded-2xl bg-slate-950 border-2 border-slate-800 flex flex-col items-center justify-center relative z-10 transition-all duration-500 shadow-2xl';
    chargerIcon.setAttribute('class', 'w-8 h-8 md:w-10 md:h-10 text-slate-600 transition-colors duration-500');
    carIcon.setAttribute('class', 'w-full h-full text-slate-700 transition-all duration-500 drop-shadow-2xl');
    carBox.classList.remove('scale-105');

    const animWire1 = document.getElementById('animWire1');
    const animWire2 = document.getElementById('animWire2');
    if (animWire1) { animWire1.classList.add('opacity-0'); animWire1.classList.remove('anim-flow'); animWire1.setAttribute('stroke', '#10b981'); }
    if (animWire2) { animWire2.classList.add('opacity-0'); animWire2.classList.remove('anim-flow'); animWire2.setAttribute('stroke', '#10b981'); }
    const isSchukoState = greenup_state === 'F' || greenup_state === 'G' || greenup_state === 'H' || greenup_state === 'O';
    if (cableLabel) {
        cableLabel.innerText = isSchukoState ? 'Schuko' : 'Type 2';
        cableLabel.classList.add('opacity-0');
        cableLabel.classList.remove('text-rose-500', 'text-sky-400');
        cableLabel.classList.add('text-emerald-400');
    }

    carIcon.style.fill = '';
    carIcon.classList.remove('animate-pulse', 'pulse-car-blue', 'car-charging-shimmer');
    chargerBox.classList.remove('animate-pulse', 'pulse-charger-blue', 'pulse-charger-green');

    let stateLabel = greenup_state || iec_state;
    const isFaultState = ['R', 'S', 'T', 'U', 'V', 'X', 'ST'].includes(greenup_state) ||
        ((iec_state === 'E' || iec_state === 'F') && !['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P'].includes(greenup_state));

    // 1. Fault states: Red visualizer
    if (isFaultState) {
        stateLabel = `${t('state.fault')} (${greenup_state || iec_state}) - ${t('state.code')}: ${error_code || t('state.unknown')}`;
        chargerBox.classList.add('border-rose-500/50', 'shadow-[0_0_30px_rgba(244,63,94,0.15)]');
        chargerIcon.classList.replace('text-slate-600', 'text-rose-500');
        updateChargerLeds('static-red');

        carIcon.classList.replace('text-slate-700', 'text-rose-500');

        if (animWire1) { animWire1.classList.remove('opacity-0'); animWire1.setAttribute('stroke', '#f43f5e'); }
        if (animWire2) { animWire2.classList.remove('opacity-0'); animWire2.setAttribute('stroke', '#f43f5e'); }
        if (cableLabel) {
            cableLabel.classList.remove('opacity-0', 'text-emerald-400', 'text-sky-400');
            cableLabel.classList.add('text-rose-500');
        }
    }
    // 2. Disconnected states (Legrand A or L)
    else if (greenup_state === 'A' || greenup_state === 'L' || greenup_state === 'DISCONNECTED') {
        stateLabel = t('state.disconnected') + " (" + greenup_state + ")";
        updateChargerLeds('static-white');
    }
    // 3. Active charging states (Legrand D, E for Type 2, or G, H for Schuko / Domestic)
    else if (greenup_state === 'D' || greenup_state === 'E' || greenup_state === 'G' || greenup_state === 'H') {
        stateLabel = t('state.charging') + " (" + greenup_state + ")";

        // Flowing green cable
        if (animWire1) {
            animWire1.classList.remove('opacity-0');
            animWire1.classList.add('anim-flow');
            animWire1.setAttribute('stroke', '#10b981');
        }
        if (animWire2) {
            animWire2.classList.remove('opacity-0');
            animWire2.classList.add('anim-flow');
            animWire2.setAttribute('stroke', '#10b981');
        }
        if (cableLabel) {
            cableLabel.classList.remove('opacity-0', 'text-rose-500', 'text-sky-400');
            cableLabel.classList.add('text-emerald-400');
        }

        // Green charger with gentle breathing pulse and electric blue lightning bolt
        chargerBox.classList.add('border-emerald-500/50', 'pulse-charger-green');
        chargerIcon.classList.replace('text-slate-600', 'text-sky-400');
        updateChargerLeds('chase-green');

        // Animated green shimmer car (Right-to-Left energy flow from charger)
        carBox.classList.add('scale-105');
        carIcon.classList.add('car-charging-shimmer');
    }
    // 4. Legrand State B (Type 2) or F (Schuko / Domestic): Vehicle connected, waiting for station authorization
    else if (greenup_state === 'B' || greenup_state === 'F') {
        stateLabel = (t('state.connected_waiting_station') || (t('state.connected') + " — Attente borne")) + " (" + greenup_state + ")";

        // Solid blue cable (connected)
        if (animWire1) { animWire1.classList.remove('opacity-0'); animWire1.classList.remove('anim-flow'); animWire1.setAttribute('stroke', '#38bdf8'); }
        if (animWire2) { animWire2.classList.remove('opacity-0'); animWire2.classList.remove('anim-flow'); animWire2.setAttribute('stroke', '#38bdf8'); }
        if (cableLabel) {
            cableLabel.classList.remove('opacity-0', 'text-rose-500', 'text-emerald-400');
            cableLabel.classList.add('text-sky-400');
        }

        // Charger pulses slowly in blue
        chargerBox.classList.add('border-sky-500/60', 'pulse-charger-blue');
        chargerIcon.classList.replace('text-slate-600', 'text-sky-400');
        updateChargerLeds('chase-white');

        // Car remains idle slate (ready)
    }
    // 5. Legrand State C: Station authorized and ready, waiting for EV to request charge
    else if (greenup_state === 'C') {
        stateLabel = (t('state.connected_waiting_car') || (t('state.connected') + " — Attente véhicule")) + " (C)";

        // Solid blue cable (connected)
        if (animWire1) { animWire1.classList.remove('opacity-0'); animWire1.classList.remove('anim-flow'); animWire1.setAttribute('stroke', '#38bdf8'); }
        if (animWire2) { animWire2.classList.remove('opacity-0'); animWire2.classList.remove('anim-flow'); animWire2.setAttribute('stroke', '#38bdf8'); }
        if (cableLabel) {
            cableLabel.classList.remove('opacity-0', 'text-rose-500', 'text-emerald-400');
            cableLabel.classList.add('text-sky-400');
        }

        // Charger is solid green (ready & authorized)
        chargerBox.classList.add('border-emerald-500/50', 'shadow-[0_0_30px_rgba(16,185,129,0.15)]');
        chargerIcon.classList.replace('text-slate-600', 'text-sky-400');
        updateChargerLeds('chase-white');

        // Car pulses slowly in blue
        carIcon.style.fill = '#38bdf8';
        carIcon.classList.add('pulse-car-blue');
    }
    // 6. Other connected states (I, J, K, M, N, O, P, W...)
    else {
        stateLabel = t('state.connected') + " (" + greenup_state + ")";
        if (animWire1) { animWire1.classList.remove('opacity-0'); animWire1.classList.remove('anim-flow'); animWire1.setAttribute('stroke', '#38bdf8'); }
        if (animWire2) { animWire2.classList.remove('opacity-0'); animWire2.classList.remove('anim-flow'); animWire2.setAttribute('stroke', '#38bdf8'); }
        if (cableLabel) {
            cableLabel.classList.remove('opacity-0', 'text-rose-500', 'text-emerald-400');
            cableLabel.classList.add('text-sky-400');
        }
        updateChargerLeds('static-white');
    }

    // Special case: charge physically completed (battery full)
    if (isChargeComplete && ['A', 'B', 'C', 'F', 'I'].includes(greenup_state)) {
        stateLabel = t('state.charge_complete') + " (" + greenup_state + ")";
        carIcon.classList.remove('pulse-car-blue', 'car-charging-shimmer');
        carIcon.classList.replace('text-slate-700', 'text-emerald-500');
        carIcon.classList.replace('text-sky-500', 'text-emerald-500');
        carIcon.style.fill = '#10b981';
    }

    updateControlButtons(charge_authorized, charge_paused);

    lblState.innerText = stateLabel;
}
