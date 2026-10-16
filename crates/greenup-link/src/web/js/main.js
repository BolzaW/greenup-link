// Entry point: language initialization, authentication check, and telemetry polling loop.

const savedLang = localStorage.getItem('prefLang') || (navigator.language.startsWith('fr') ? 'fr' : 'en');
document.getElementById('langSelector').value = savedLang;

setupAuthModalListeners();

setLanguage(savedLang).then(() => {
    checkAuthAndStart();
});

setInterval(fetchTelemetry, 2000);
