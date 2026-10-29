// Internationalization: loads /locales/{lang}.json and translates [data-i18n] elements.

let currentTranslations = {};
const t = (key) => key.split('.').reduce((obj, k) => (obj || {})[k], currentTranslations) || key;

async function setLanguage(lang) {
    try {
        const response = await fetch('/locales/' + lang + '.json');
        currentTranslations = await response.json();
        localStorage.setItem('prefLang', lang);

        document.querySelectorAll('[data-i18n]').forEach(el => {
            const translation = t(el.getAttribute('data-i18n'));
            if (translation && translation !== el.getAttribute('data-i18n')) el.innerText = translation;
        });

        document.querySelectorAll('[data-i18n-title]').forEach(el => {
            const translation = t(el.getAttribute('data-i18n-title'));
            if (translation && translation !== el.getAttribute('data-i18n-title')) el.title = translation;
        });

        document.querySelectorAll('[data-i18n-placeholder]').forEach(el => {
            const translation = t(el.getAttribute('data-i18n-placeholder'));
            if (translation && translation !== el.getAttribute('data-i18n-placeholder')) el.placeholder = translation;
        });

        if (lastReportedTicMode) updateLinky(lastReportedTicMode);
        window.lastVisualState = null; // force visual update on next poll
        if (typeof updateLogoutButton === 'function') updateLogoutButton();
    } catch(err) { console.error('Failed to load translations', err); }
}

document.getElementById('langSelector').addEventListener('change', (e) => {
    setLanguage(e.target.value).then(() => {
        if (typeof fetchTelemetry === 'function') fetchTelemetry();
    });
});
