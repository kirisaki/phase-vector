import type { State } from './types.js';
import { bindDialogDismiss } from './dialog.js';
import { loadTurnstile } from './feedback-turnstile.js';
import { getDialogLanguage, setDialogLanguage, onDialogLanguageChange } from './dialog-language.js';
import { feedbackText, type FeedbackTextKey } from './feedback-content.js';

interface Config { version: string; feedbackApiUrl: string; turnstileSiteKey: string }
const get = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const sessionKey = 'phase-vector.feedback-session';
function randomId(): string {
  // getRandomValues also works on HTTP LAN previews, unlike randomUUID.
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  bytes[6] = (bytes[6] & 15) | 64; bytes[8] = (bytes[8] & 63) | 128;
  const hex = [...bytes].map(b => b.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0,8)}-${hex.slice(8,12)}-${hex.slice(12,16)}-${hex.slice(16,20)}-${hex.slice(20)}`;
}
function sessionId(): string {
  try {
    const saved = sessionStorage.getItem(sessionKey);
    if (saved && /^[0-9a-f-]{36}$/i.test(saved)) return saved;
    const id = randomId(); sessionStorage.setItem(sessionKey, id); return id;
  } catch { return randomId(); }
}
// Explicit allowlist: never serialize a whole State, enemy tracks, console text,
// browser URLs, or the detailed score object (which includes true enemy fuel).
export function feedbackMetadata(state: State, version: string) {
  const own = state.world.ships.find(ship => ship.id === state.player_id);
  const selected = get<HTMLInputElement>('forecast-time');
  const live = get<HTMLButtonElement>('forecast-live').getAttribute('aria-pressed') === 'true';
  return {
    version, outcome: state.mission?.outcome ?? 'sandbox', runKind: state.mission?.run_kind ?? 'sandbox',
    simulationSeconds: state.time_seconds, forecastSeconds: live ? null : Number(selected.getAttribute('aria-valuenow')),
    score: state.mission?.evaluation?.total ?? null, shotsFired: state.mission?.shots.length ?? 0,
    ownDeltaVSpent: own?.spent_dv ?? 0, viewport: { width: innerWidth, height: innerHeight },
  };
}
export function createFeedback(read: () => State, pause: () => void) {
  // Preserve the WebGL drawing buffer at its first creation. A later DOM capture
  // cannot reliably read a framebuffer that the compositor has already cleared.
  const canvas = get<HTMLCanvasElement>('tactical-map');
  const nativeGetContext = canvas.getContext;
  canvas.getContext = ((kind: string, options?: object) => {
    const context = Reflect.apply(nativeGetContext, canvas, [kind,
      kind === 'webgl2' || kind === 'webgl' ? { ...options, preserveDrawingBuffer: true } : options]);
    if (context && (kind === 'webgl2' || kind === 'webgl')) canvas.getContext = nativeGetContext;
    return context;
  }) as HTMLCanvasElement['getContext'];

  const dialog = get<HTMLDialogElement>('feedback-dialog'), opener = get<HTMLButtonElement>('feedback-open');
  const form = get<HTMLFormElement>('feedback-form'), submit = get<HTMLButtonElement>('feedback-send');
  const status = get<HTMLParagraphElement>('feedback-status'), attach = get<HTMLInputElement>('feedback-attach');
  const preview = get<HTMLImageElement>('feedback-preview'), details = get<HTMLDListElement>('feedback-metadata');
  const privacy = get<HTMLInputElement>('feedback-consent'), verifyButton = get<HTMLButtonElement>('feedback-verify');
  const languageButtons = dialog.querySelectorAll<HTMLButtonElement>('[data-feedback-language]');
  const tabs = [...dialog.querySelectorAll<HTMLButtonElement>('[data-feedback-tab]')];
  let language = getDialogLanguage(), statusKey: FeedbackTextKey = 'capturing';
  const t = (key: FeedbackTextKey) => feedbackText(language, key);
  let config: Config = { version: 'development', feedbackApiUrl: '', turnstileSiteKey: '' };
  let screenshot: Blob | null = null, imageUrl = '', token = '', widget: string | null = null;
  let metadata: ReturnType<typeof feedbackMetadata> | null = null;
  let opening = false, sending = false, sent = false, generation = 0;
  let submissionId = randomId(), lastPayload = '';
  let returnFocus: HTMLElement = opener;
  const anonymousId = sessionId();
  function showTab(tab: 'report' | 'review') {
    get('feedback-entry').hidden = tab !== 'report';
    get('feedback-review').hidden = tab !== 'review';
    for (const button of tabs) {
      const selected = button.dataset.feedbackTab === tab;
      button.setAttribute('aria-selected', String(selected)); button.tabIndex = selected ? 0 : -1;
    }
  }
  for (const button of tabs) {
    button.addEventListener('click', () => showTab(button.dataset.feedbackTab as 'report' | 'review'));
    button.addEventListener('keydown', event => {
      if (!['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) return;
      event.preventDefault();
      const next = event.key === 'Home' ? tabs[0] : event.key === 'End' ? tabs[1] : tabs[1 - tabs.indexOf(button)];
      next.click(); next.focus();
    });
  }
  function renderMetadata() {
    details.replaceChildren();
    if (!metadata) return;
    const values = { ...metadata, sessionId: anonymousId };
    for (const [key, value] of Object.entries(values)) {
      const item = document.createElement('div'), label = document.createElement('dt'), display = document.createElement('dd');
      item.dataset.metadataKey = key; item.dataset.value = JSON.stringify(value);
      label.textContent = t(key as FeedbackTextKey);
      display.textContent = key === 'outcome' || key === 'runKind' ? t(value as FeedbackTextKey)
        : key === 'viewport' ? `${metadata.viewport.width} × ${metadata.viewport.height}`
        : value === null ? t(key === 'forecastSeconds' ? 'live' : 'unavailable')
        : typeof value === 'number' ? new Intl.NumberFormat(language, { maximumFractionDigits: 3 }).format(value) : String(value);
      item.append(label, display); details.append(item);
    }
  }
  function renderStatus() { status.textContent = t(statusKey) + (!configured() ? ` ${t('unconfigured')}` : ''); }
  function setStatus(key: FeedbackTextKey) { statusKey = key; renderStatus(); }
  function renderLanguage() {
    dialog.lang = language;
    for (const node of dialog.querySelectorAll<HTMLElement>('[data-feedback-text]')) node.textContent = t(node.dataset.feedbackText as FeedbackTextKey);
    for (const button of languageButtons) button.setAttribute('aria-pressed', String(button.dataset.feedbackLanguage === language));
    get('feedback-close').setAttribute('aria-label', t('close'));
    get('feedback-tabs').setAttribute('aria-label', t('title'));
    get<HTMLTextAreaElement>('feedback-message').placeholder = t('placeholder');
    preview.alt = t('preview');
    renderMetadata(); renderStatus();
  }
  const configuration = fetch(new URL('./runtime-config.json', location.href), { cache: 'no-store' })
    .then(response => { if (!response.ok) throw new Error('config'); return response.json() as Promise<Config>; })
    .then(value => {
      if (typeof value.version !== 'string' || typeof value.feedbackApiUrl !== 'string' || typeof value.turnstileSiteKey !== 'string') return;
      if (value.feedbackApiUrl) {
        const url = new URL(value.feedbackApiUrl);
        if (url.username || url.password || url.search || url.hash || url.pathname !== '/'
          || (url.protocol !== 'https:' && !(url.protocol === 'http:' && ['localhost','127.0.0.1'].includes(url.hostname)))) return;
      }
      config = { ...value, feedbackApiUrl: value.feedbackApiUrl.trim(), turnstileSiteKey: value.turnstileSiteKey.trim() };
    }).catch(() => { /* Missing configuration must never stop the game. */ });
  const configured = () => Boolean(config.feedbackApiUrl && config.turnstileSiteKey);
  function sync() {
    get<HTMLFieldSetElement>('feedback-fields').disabled = sending || sent;
    submit.disabled = sending || sent || !metadata || !configured() || !privacy.checked || !token;
    verifyButton.disabled = sending || sent || !configured() || !privacy.checked;
    attach.disabled = !screenshot || sending || sent;
    for (const button of languageButtons) button.disabled = sending;
  }
  function releaseImage() {
    if (imageUrl) URL.revokeObjectURL(imageUrl);
    imageUrl = ''; screenshot = null; preview.removeAttribute('src'); preview.hidden = true;
    attach.checked = false;
  }
  function removeWidget() {
    generation++; token = '';
    if (widget !== null) window.turnstile?.remove(widget);
    widget = null;
  }
  function close() {
    if (sending) return;
    dialog.close(); removeWidget(); releaseImage(); returnFocus.focus();
  }
  bindDialogDismiss(dialog, close);
  get<HTMLButtonElement>('feedback-close').addEventListener('click', close);
  get<HTMLButtonElement>('feedback-remove-image').addEventListener('click', () => { releaseImage(); sync(); });
  form.addEventListener('input', sync);
  privacy.addEventListener('change', () => { if (!privacy.checked) removeWidget(); sync(); });
  async function open(source: HTMLButtonElement) {
    if (opening || dialog.open) return;
    returnFocus = source; opening = true; opener.disabled = true; opener.setAttribute('aria-busy', 'true');
    try {
      await configuration; pause();
      metadata = feedbackMetadata(read(), config.version);
      removeWidget(); releaseImage(); sent = false; form.reset(); submissionId = randomId(); lastPayload = '';
      showTab('report'); renderMetadata();
      setStatus('capturing');
      let captureMessage: FeedbackTextKey;
      try {
        const { captureApplication } = await import('./feedback-capture.js');
        screenshot = await captureApplication();
        imageUrl = URL.createObjectURL(screenshot); preview.src = imageUrl; preview.hidden = false;
        captureMessage = 'captured';
      } catch (error) { console.warn('Screenshot capture unavailable', error); captureMessage = 'captureFailed'; }
      // The dialog is opened only after capture, so it cannot cover the game image.
      dialog.showModal();
      dialog.scrollTop = 0;
      setStatus(captureMessage);
      sync();
    } catch { setStatus('openFailed'); dialog.showModal(); }
    finally { opening = false; opener.disabled = false; opener.setAttribute('aria-busy', 'false'); }
  }
  opener.addEventListener('click', () => void open(opener));
  const resultOpener = get<HTMLButtonElement>('result-feedback');
  resultOpener.addEventListener('click', () => void open(resultOpener));
  verifyButton.addEventListener('click', async () => {
    if (!privacy.checked || !configured()) return;
    removeWidget(); const current = generation;
    setStatus('loading'); verifyButton.disabled = true;
    try {
      const turnstile = await loadTurnstile();
      if (!dialog.open || current !== generation || !privacy.checked) return;
      widget = turnstile.render(get('feedback-turnstile'), {
        sitekey: config.turnstileSiteKey, action: 'feedback', theme: 'dark', language, size: innerWidth < 400 ? 'compact' : 'normal',
        callback: value => { if (current === generation) { token = value; setStatus('verified'); sync(); } },
        'expired-callback': () => { if (current === generation) { token = ''; setStatus('expired'); sync(); } },
        'error-callback': () => { if (current === generation) { token = ''; setStatus('verificationFailed'); sync(); } },
      });
    } catch { if (current === generation) { setStatus('verificationFailed'); sync(); } }
  });
  form.addEventListener('submit', async event => {
    event.preventDefault();
    if (submit.disabled || !metadata) return;
    if (!form.checkValidity()) { showTab('report'); form.reportValidity(); return; }
    const fields = new FormData(form);
    const payload = { sessionId: anonymousId, submissionId,
      category: String(fields.get('category')), message: String(fields.get('message')).trim(), metadata };
    // Keep a stable id for unchanged retries after an uncertain network response.
    const signature = JSON.stringify({ ...payload, submissionId: '', image: attach.checked && !!screenshot });
    if (lastPayload && signature !== lastPayload) { submissionId = randomId(); payload.submissionId = submissionId; }
    lastPayload = signature;
    const body = new FormData(); body.set('payload', JSON.stringify(payload)); body.set('token', token);
    if (attach.checked && screenshot) body.set('screenshot', screenshot, 'phase-vector.png');
    sending = true; sync(); get<HTMLButtonElement>('feedback-close').disabled = true;
    setStatus('sending');
    let failure: FeedbackTextKey = 'interrupted';
    try {
      const response = await fetch(`${config.feedbackApiUrl.replace(/\/$/,'')}/v1/feedback`, {
        method: 'POST', body, credentials: 'omit', referrerPolicy: 'no-referrer', signal: AbortSignal.timeout(20000),
      });
      if (!response.ok) {
        failure = response.status === 429 ? 'rateLimited' : response.status === 409 ? 'pending' : 'failed';
        throw new Error('Submission failed');
      }
      sent = true; setStatus('sent'); removeWidget(); releaseImage();
    } catch {
      setStatus(failure);
      removeWidget();
    } finally { sending = false; get<HTMLButtonElement>('feedback-close').disabled = false; sync(); }
  });
  for (const button of languageButtons) button.addEventListener('click', () => {
    const next = button.dataset.feedbackLanguage;
    if (next === 'ja' || next === 'en') setDialogLanguage(next);
  });
  onDialogLanguageChange(next => {
    language = next;
    if (widget !== null || statusKey === 'loading') { removeWidget(); statusKey = 'verifyAgain'; }
    renderLanguage(); sync();
  });
  renderLanguage();
}
