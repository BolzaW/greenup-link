// User controls: Enable/Disable & Pause/Resume buttons, current slider, panel locking.

function lockControls() {
    isActionPending = true;
    controlsPanel.classList.add('opacity-50', 'pointer-events-none');
}

function unlockControls() {
    isActionPending = false;
    controlsPanel.classList.remove('opacity-50', 'pointer-events-none');
    // Let polling restore state properly
}

function updateControlButtons(charge_authorized, charge_paused) {
    const iconEnable = document.getElementById('iconEnable');
    const iconDisable = document.getElementById('iconDisable');
    const lblEnableDisable = document.getElementById('lblEnableDisable');
    const iconPause = document.getElementById('iconPause');
    const iconResume = document.getElementById('iconResume');
    const lblPauseResume = document.getElementById('lblPauseResume');

    if (isActionPending) {
        btnEnableDisable.classList.add('opacity-50', 'pointer-events-none');
        btnPauseResume.classList.add('opacity-50', 'pointer-events-none');
    } else {
        btnEnableDisable.classList.remove('opacity-50', 'pointer-events-none');
        btnPauseResume.classList.remove('opacity-50', 'pointer-events-none');
    }

    if (charge_authorized) {
        iconEnable.classList.add('hidden');
        iconDisable.classList.remove('hidden');
        lblEnableDisable.innerText = t('controls.disable');
        btnEnableDisable.classList.replace('bg-emerald-500', 'bg-rose-500');
        btnEnableDisable.classList.replace('hover:bg-emerald-400', 'hover:bg-rose-400');
        btnEnableDisable.classList.replace('text-emerald-950', 'text-white');
        btnEnableDisable.classList.replace('shadow-[0_0_20px_rgba(16,185,129,0.2)]', 'shadow-[0_0_20px_rgba(244,63,94,0.2)]');
        btnEnableDisable.classList.replace('hover:shadow-[0_0_25px_rgba(16,185,129,0.4)]', 'hover:shadow-[0_0_25px_rgba(244,63,94,0.4)]');
    } else {
        iconDisable.classList.add('hidden');
        iconEnable.classList.remove('hidden');
        lblEnableDisable.innerText = t('controls.enable');
        btnEnableDisable.classList.replace('bg-rose-500', 'bg-emerald-500');
        btnEnableDisable.classList.replace('hover:bg-rose-400', 'hover:bg-emerald-400');
        btnEnableDisable.classList.replace('text-white', 'text-emerald-950');
        btnEnableDisable.classList.replace('shadow-[0_0_20px_rgba(244,63,94,0.2)]', 'shadow-[0_0_20px_rgba(16,185,129,0.2)]');
        btnEnableDisable.classList.replace('hover:shadow-[0_0_25px_rgba(244,63,94,0.4)]', 'hover:shadow-[0_0_25px_rgba(16,185,129,0.4)]');
    }

    if (charge_paused) {
        iconPause.classList.add('hidden');
        iconResume.classList.remove('hidden');
        lblPauseResume.innerText = t('controls.resume');
        btnPauseResume.classList.replace('bg-amber-500', 'bg-emerald-500');
        btnPauseResume.classList.replace('hover:bg-amber-400', 'hover:bg-emerald-400');
        btnPauseResume.classList.replace('text-amber-950', 'text-emerald-950');
        btnPauseResume.classList.replace('shadow-[0_0_20px_rgba(245,158,11,0.2)]', 'shadow-[0_0_20px_rgba(16,185,129,0.2)]');
        btnPauseResume.classList.replace('hover:shadow-[0_0_25px_rgba(245,158,11,0.4)]', 'hover:shadow-[0_0_25px_rgba(16,185,129,0.4)]');
    } else {
        iconResume.classList.add('hidden');
        iconPause.classList.remove('hidden');
        lblPauseResume.innerText = t('controls.pause');
        btnPauseResume.classList.replace('bg-emerald-500', 'bg-amber-500');
        btnPauseResume.classList.replace('hover:bg-emerald-400', 'hover:bg-amber-400');
        btnPauseResume.classList.replace('text-emerald-950', 'text-amber-950');
        btnPauseResume.classList.replace('shadow-[0_0_20px_rgba(16,185,129,0.2)]', 'shadow-[0_0_20px_rgba(245,158,11,0.2)]');
        btnPauseResume.classList.replace('hover:shadow-[0_0_25px_rgba(16,185,129,0.4)]', 'hover:shadow-[0_0_25px_rgba(245,158,11,0.4)]');
    }
}

btnEnableDisable.addEventListener('click', () => sendPostAction('/api/charge', { action: window.lastChargeAuthorized ? 'disable' : 'enable' }));

btnPauseResume.addEventListener('click', () => sendPostAction('/api/charge', { action: window.lastChargePaused ? 'resume' : 'pause' }));

slider.addEventListener('mousedown', () => isDraggingSlider = true);
slider.addEventListener('mouseup', () => isDraggingSlider = false);
slider.addEventListener('touchstart', () => isDraggingSlider = true);
slider.addEventListener('touchend', () => isDraggingSlider = false);

slider.addEventListener('input', (e) => {
    lblSlider.innerText = e.target.value + " A";
});
slider.addEventListener('change', (e) => {
    sendPostAction('/api/current/' + e.target.value);
});
