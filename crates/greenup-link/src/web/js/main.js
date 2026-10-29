// Entry point: language initialization, authentication check, and lifecycle start.

const savedLang = localStorage.getItem('prefLang') || (navigator.language.startsWith('fr') ? 'fr' : 'en');
const langSelector = document.getElementById('langSelector');
if (langSelector) langSelector.value = savedLang;

setupAuthModalListeners();

// Initialize i18n and check authentication status concurrently
Promise.all([
    setLanguage(savedLang),
    checkAuthAndStart()
]).catch(err => {
    console.error('Initialization error:', err);
});
