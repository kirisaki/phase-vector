export type DialogLanguage = 'ja' | 'en';
const cookieName = 'phase-vector.language';
function saveLanguage(language: DialogLanguage) {
  try {
    const path = new URL('.', location.href).pathname;
    document.cookie = `${cookieName}=${language}; Path=${path}; Max-Age=31536000; SameSite=Lax${location.protocol === 'https:' ? '; Secure' : ''}`;
  } catch { /* Retain the in-memory choice when cookies are unavailable. */ }
}
function initialLanguage(): DialogLanguage {
  try {
    const saved = document.cookie.split(';').map(part => part.trim()).find(part => part.startsWith(`${cookieName}=`))?.split('=')[1];
    if (saved === 'ja' || saved === 'en') return saved;
    // Preserve an existing Help preference once, then use only the shared cookie.
    const legacy = localStorage.getItem('phase-vector.help-language');
    if (legacy === 'ja' || legacy === 'en') {
      saveLanguage(legacy);
      localStorage.removeItem('phase-vector.help-language');
      return legacy;
    }
  } catch { /* Dialogs remain usable when browser storage is unavailable. */ }
  return /^ja(?:-|$)/i.test(navigator.language) ? 'ja' : 'en';
}
let language = initialLanguage();
const listeners = new Set<(language: DialogLanguage) => void>();
export const getDialogLanguage = () => language;
export function setDialogLanguage(next: DialogLanguage) {
  saveLanguage(next);
  if (language === next) return;
  language = next;
  for (const listener of listeners) listener(next);
}
export function onDialogLanguageChange(listener: (language: DialogLanguage) => void) {
  listeners.add(listener);
}
