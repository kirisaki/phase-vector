import { element } from './dom.js';
import { helpContent, helpLabels, type HelpLanguage, type HelpTopic } from './help-content.js';

const languageKey = 'phase-vector.help-language';

function initialLanguage(): HelpLanguage {
  try {
    const saved = localStorage.getItem(languageKey);
    if (saved === 'ja' || saved === 'en') return saved;
  } catch { /* Help remains usable when browser storage is unavailable. */ }
  return /^ja(?:-|$)/i.test(navigator.language) ? 'ja' : 'en';
}

export function createHelp(onOpen: () => void, onInsert: (source: string) => void) {
  const dialog = element('help-dialog'), body = element('help-body');
  const opener = element('help-open');
  const languages = dialog.querySelectorAll<HTMLButtonElement>('[data-help-language]');
  const topics = dialog.querySelectorAll<HTMLButtonElement>('[data-help-topic]');
  let language = initialLanguage(), topic: HelpTopic = 'quick', ready = false;
  const expanded = new Map<string, boolean>();
  const positions: Record<HelpTopic, number> = { quick: 0, controls: 0, lua: 0 };

  function textNode<K extends keyof HTMLElementTagNameMap>(tag: K, text: string) {
    const node = document.createElement(tag);
    node.textContent = text;
    return node;
  }

  function remember() {
    positions[topic] = body.scrollTop;
    for (const details of body.querySelectorAll<HTMLDetailsElement>('details[data-help-group]')) {
      expanded.set(details.dataset.helpGroup ?? '', details.open);
    }
  }

  function close() { dialog.close(); }

  function syncInsertButtons() {
    for (const button of body.querySelectorAll<HTMLButtonElement>('[data-help-insert]')) {
      button.disabled = !ready;
      button.title = ready ? helpLabels.insert[language] : helpLabels.loading[language];
    }
  }

  function render() {
    dialog.lang = language;
    element('help-title').textContent = helpLabels.title[language];
    element('help-close').setAttribute('aria-label', helpLabels.close[language]);
    element('help-pause-note').textContent = helpLabels.paused[language];
    element('help-topics').setAttribute('aria-label', helpLabels.topics[language]);
    for (const button of languages) button.setAttribute('aria-pressed', String(button.dataset.helpLanguage === language));
    for (const button of topics) {
      const name = button.dataset.helpTopic as HelpTopic;
      button.textContent = helpContent[name].title[language];
      button.setAttribute('aria-pressed', String(name === topic));
    }
    const section = helpContent[topic];
    body.setAttribute('aria-label', section.title[language]);
    body.dataset.topic = topic;
    body.replaceChildren(textNode('h3', section.title[language]), textNode('p', section.intro[language]));
    if (section.steps) {
      const steps = document.createElement('ol');
      for (const entry of section.steps) {
        const item = document.createElement('li');
        item.append(textNode('h4', entry.title[language]), textNode('p', entry.text[language]));
        steps.append(item);
      }
      body.append(steps);
    }
    for (const group of section.groups ?? []) {
      const details = document.createElement('details');
      details.dataset.helpGroup = group.id;
      details.open = expanded.get(group.id) ?? group.open ?? false;
      details.append(textNode('summary', group.title[language]));
      for (const entry of group.entries) {
        const article = document.createElement('article');
        article.append(textNode('h4', entry.title[language]), textNode('p', entry.text[language]));
        const source = entry.code;
        if (source !== undefined) {
          const pre = document.createElement('pre');
          pre.append(textNode('code', source));
          const insert = textNode('button', helpLabels.insert[language]);
          insert.type = 'button';
          insert.dataset.helpInsert = source;
          insert.addEventListener('click', () => {
            if (!ready) return;
            close();
            onInsert(source);
          });
          article.append(pre, insert);
        }
        details.append(article);
      }
      body.append(details);
    }
    syncInsertButtons();
    body.scrollTop = positions[topic];
  }

  for (const button of languages) button.addEventListener('click', () => {
    const next = button.dataset.helpLanguage;
    if (next !== 'ja' && next !== 'en') return;
    remember(); language = next;
    try { localStorage.setItem(languageKey, language); } catch { /* Retain the session choice. */ }
    render();
  });
  for (const button of topics) button.addEventListener('click', () => {
    const next = button.dataset.helpTopic;
    if (next !== 'quick' && next !== 'controls' && next !== 'lua') return;
    remember(); topic = next; render();
  });
  opener.addEventListener('click', () => {
    if (dialog.open) return;
    onOpen();
    dialog.showModal();
    body.scrollTop = positions[topic];
  });
  element('help-close').addEventListener('click', close);
  dialog.addEventListener('cancel', (event) => { event.preventDefault(); close(); });
  dialog.addEventListener('close', () => {
    remember();
    // Do not steal focus from a Lua example inserted immediately after closing.
    if (document.activeElement === document.body || dialog.contains(document.activeElement)) opener.focus();
  });
  const outside = (event: MouseEvent) => {
    const box = dialog.getBoundingClientRect();
    return event.clientX < box.left || event.clientX > box.right || event.clientY < box.top || event.clientY > box.bottom;
  };
  let pressedOutside = false;
  dialog.addEventListener('pointerdown', (event) => { pressedOutside = event.target === dialog && outside(event); });
  dialog.addEventListener('click', (event) => {
    if (pressedOutside && event.target === dialog && outside(event)) close();
    pressedOutside = false;
  });
  render();
  return { setReady(value: boolean) {
    if (ready === value) return;
    ready = value; syncInsertButtons();
  } };
}
