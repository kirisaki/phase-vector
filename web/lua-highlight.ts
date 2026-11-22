// Lexical coloring only; the embedded Lua interpreter remains the syntax authority.
const keywords = new Set('and break do else elseif end false for function goto if in local nil not or repeat return then true until while'.split(' '));
type Kind = 'plain' | 'keyword' | 'string' | 'comment' | 'number';
interface Token { kind: Kind; text: string }

function longEnd(source: string, start: number): number | null {
  const opening = /^\[(=*)\[/.exec(source.slice(start));
  if (!opening) return null;
  const closing = `]${opening[1]}]`;
  const end = source.indexOf(closing, start + opening[0].length);
  return end < 0 ? source.length : end + closing.length;
}

function quotedEnd(source: string, start: number): number {
  const quote = source[start];
  let end = start + 1;
  while (end < source.length) {
    const char = source[end];
    if (char === quote) return end + 1;
    if (char === '\n' || char === '\r') break;
    if (char === '\\') {
      if (source[end + 1] === 'z') {
        end += 2;
        while (/\s/.test(source[end] ?? '')) end++;
      } else {
        end += source[end + 1] === '\r' && source[end + 2] === '\n' ? 3 : 2;
      }
    } else end++;
  }
  return Math.min(end, source.length);
}

export function luaTokens(source: string): Token[] {
  const tokens: Token[] = [];
  // Consume whole names and concatenation operators so their suffixes are not numbers.
  const next = /--|\[(=*)\[|["']|\.{2,3}|0[xX](?:[\da-fA-F]+(?:\.(?!\.)[\da-fA-F]*)?|\.[\da-fA-F]+)(?:[pP][+-]?\d+)?|(?:\d+(?:\.(?!\.)\d*)?|\.\d+)(?:[eE][+-]?\d+)?|[A-Za-z_][A-Za-z_\d]*/g;
  let consumed = 0;
  for (let match = next.exec(source); match; match = next.exec(source)) {
    const start = match.index, text = match[0];
    if (start > consumed) tokens.push({ kind: 'plain', text: source.slice(consumed, start) });
    let end = next.lastIndex, kind: Kind = 'plain';
    if (text === '--') {
      kind = 'comment';
      const long = longEnd(source, end);
      const newline = source.slice(end).search(/[\r\n]/);
      end = long ?? (newline < 0 ? source.length : end + newline);
    } else if (text[0] === '[') {
      kind = 'string'; end = longEnd(source, start) ?? end;
    } else if (text === '"' || text === "'") {
      kind = 'string'; end = quotedEnd(source, start);
    } else if (keywords.has(text)) kind = 'keyword';
    else if (/^(?:\d|\.\d)/.test(text)) kind = 'number';
    tokens.push({ kind, text: source.slice(start, end) });
    consumed = next.lastIndex = end;
  }
  if (consumed < source.length) tokens.push({ kind: 'plain', text: source.slice(consumed) });
  return tokens;
}

export function createLuaHighlight(input: HTMLTextAreaElement, mirror: HTMLPreElement) {
  let previous: string | undefined;
  const scroll = () => { mirror.scrollTop = input.scrollTop; mirror.scrollLeft = input.scrollLeft; };
  function sync() {
    if (input.value !== previous) {
      previous = input.value;
      const fragment = document.createDocumentFragment();
      for (const token of luaTokens(previous)) {
        const node = document.createElement('span');
        node.className = `lua-${token.kind}`;
        node.textContent = token.text;
        fragment.append(node);
      }
      // Preserve the final empty line's height without inserting anything into the input.
      fragment.append(document.createTextNode('\u200b'));
      mirror.replaceChildren(fragment);
    }
    scroll();
  }
  input.addEventListener('input', sync);
  input.addEventListener('scroll', scroll);
  input.addEventListener('compositionstart', () => { input.dataset.composing = 'true'; });
  input.addEventListener('compositionend', () => { delete input.dataset.composing; sync(); });
  new ResizeObserver(scroll).observe(input);
  sync();
  input.dataset.highlighted = 'true';
  return { sync };
}
