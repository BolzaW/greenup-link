// Entry point: language initialization, authentication check, and lifecycle start.

const savedLang = localStorage.getItem('prefLang') || (navigator.language.startsWith('fr') ? 'fr' : 'en');
const langSelector = document.getElementById('langSelector');
if (langSelector) langSelector.value = savedLang;

setupAuthModalListeners();

// Initialize i18n first so all translations are ready, then check authentication and start
(async () => {
    try {
        await setLanguage(savedLang);
    } catch (err) {
        console.error('Language initialization error:', err);
    }
    try {
        await checkAuthAndStart();
    } catch (err) {
        console.error('Initialization error:', err);
    }
})();
