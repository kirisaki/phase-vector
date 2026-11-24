interface Turnstile {
  render(container: HTMLElement, options: { sitekey: string; action: string; theme: string; size: 'compact' | 'normal';
    callback: (token: string) => void; 'expired-callback': () => void; 'error-callback': () => void }): string;
  remove(id: string): void;
  reset(id: string): void;
}
declare global { interface Window { turnstile?: Turnstile } }
let scriptPromise: Promise<Turnstile> | undefined;
export function loadTurnstile(): Promise<Turnstile> {
  if (window.turnstile) return Promise.resolve(window.turnstile);
  if (!scriptPromise) scriptPromise = new Promise<Turnstile>((resolve, reject) => {
    const script = document.createElement('script');
    script.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit';
    script.async = true;
    const timeout = setTimeout(() => { script.remove(); scriptPromise = undefined; reject(new Error('Verification could not load')); }, 12000);
    script.onload = () => {
      clearTimeout(timeout);
      if (window.turnstile) resolve(window.turnstile);
      else { scriptPromise = undefined; reject(new Error('Verification unavailable')); }
    };
    script.onerror = () => { clearTimeout(timeout); script.remove(); scriptPromise = undefined; reject(new Error('Verification could not load')); };
    document.head.append(script);
  });
  return scriptPromise;
}
