// Entry point: language initialization and telemetry polling loop.

const savedLang = localStorage.getItem('prefLang') || (navigator.language.startsWith('fr') ? 'fr' : 'en');
document.getElementById('langSelector').value = savedLang;
setLanguage(savedLang).then(() => {
    fetchTelemetry();
    fetchInfo();
});
setInterval(fetchTelemetry, 2000);
