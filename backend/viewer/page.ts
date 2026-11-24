import { categories, type validatePayload } from '../src/validation';

export interface Report { id: string; created_at: number; payload_json: string; image_key: string | null; image_expires_at: number | null }
export interface Filters { category: string; days: string; version: string; before: string }
type Payload = ReturnType<typeof validatePayload>;
const labels: Record<string, string> = { bug: '不具合', controls: '操作', balance: 'バランス', performance: '動作・性能', other: 'その他' };
const escape = (value: unknown): string => String(value ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]!);
const date = (seconds: number): string => new Date(seconds * 1000).toISOString().replace('T', ' ').replace('.000Z', ' UTC');
function link(filters: Filters, changes: Record<string, string> = {}): string {
  const params = new URLSearchParams({ ...filters, ...changes });
  for (const [key, value] of [...params]) if (!value) params.delete(key);
  return escape(`/?${params}`);
}
function imageStatus(row: Report): string {
  if (row.image_expires_at !== null && row.image_expires_at <= Date.now() / 1000) return '画像期限切れ';
  return row.image_key ? '画像あり' : '画像なし';
}

export function renderPage({ rows, selected, filters, more, explicitSelection, nonce }: {
  rows: Report[]; selected: Report | null; filters: Filters; more: boolean; explicitSelection: boolean; nonce: string;
}): string {
  const report = selected ? JSON.parse(selected.payload_json) as Payload : null;
  const detail = selected && report ? `<a class="back" href="${link(filters)}">← 一覧に戻る</a>
    <div class="heading"><span class="tag">${escape(labels[report.category ?? ''] ?? report.category)}</span><time>${date(selected.created_at)}</time></div>
    <h2>フィードバック</h2><p class="message">${escape(report.message)}</p>
    <h3>スクリーンショット</h3>${imageStatus(selected) === '画像あり' ? `<a class="image-link" href="/image/${escape(selected.id)}" target="_blank" rel="noopener noreferrer"><img src="/image/${escape(selected.id)}" alt="投稿者が添付したゲーム画面"><span>画像を別タブで開く ↗</span></a><p class="muted">保存期限：${date(selected.image_expires_at!)}（期限内でも画像が削除済みの場合があります）</p>` : `<p class="muted">${imageStatus(selected)}</p>`}
    <h3>ゲーム情報</h3><p class="muted">投稿時の自己申告データです。時刻は秒、Δv は m/s です。</p>
    <dl>${Object.entries(report.metadata).map(([key, value]) => `<dt>${escape(key)}</dt><dd>${escape(typeof value === 'object' && value !== null ? JSON.stringify(value) : value ?? '—')}</dd>`).join('')}</dl>
    <details><summary>投稿識別子</summary><dl><dt>Report</dt><dd>${escape(selected.id)}</dd><dt>Session</dt><dd>${escape(report.sessionId)}</dd></dl></details>` : '<p class="empty">投稿を選択すると詳細を表示します。</p>';
  return `<!doctype html><html lang="ja"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><meta name="robots" content="noindex,nofollow"><title>Feedback · PHASE VECTOR</title><style nonce="${nonce}">
    :root{color-scheme:dark;font-family:system-ui,sans-serif;background:#080f14;color:#e4edf0;font-size:16px}*{box-sizing:border-box}body{margin:0}a{color:#73dfd5}button,input,select{font:inherit}a,button,input,select,summary{outline-offset:4px}header{display:flex;align-items:center;justify-content:space-between;gap:16px;padding:20px 24px;border-bottom:1px solid #29404a}h1{font-size:20px;margin:0}header small,.muted,time{color:#a6b8c0;font-size:13px}header a{font-size:14px}form{display:flex;flex-wrap:wrap;align-items:end;gap:12px;padding:18px 24px;border-bottom:1px solid #29404a}label{display:grid;gap:6px;font-size:13px}input,select,button,.button{border:1px solid #45616b;border-radius:5px;background:#12232c;color:inherit;padding:9px 12px;min-height:42px}input{max-width:100%;width:220px}button{cursor:pointer;background:#184944;color:#b6fff6}main{display:grid;grid-template-columns:minmax(280px,36%) minmax(0,1fr);max-width:1500px;margin:auto}.list{border-right:1px solid #29404a;padding:16px;min-width:0}.report{display:block;padding:16px;margin-bottom:10px;text-decoration:none;color:inherit;border:1px solid #29404a;border-radius:6px;background:#0d1920}.report:hover,.report[aria-current=true]{border-color:#69d5cc;background:#12302f}.heading{display:flex;flex-wrap:wrap;gap:10px;align-items:center}.tag{font-size:13px;color:#85e5da}.excerpt{display:-webkit-box;-webkit-line-clamp:3;-webkit-box-orient:vertical;overflow:hidden;overflow-wrap:anywhere;line-height:1.6;margin:10px 0}.report small{color:#a6b8c0;overflow-wrap:anywhere}article{padding:24px;min-width:0}h2{font-size:22px}h3{font-size:17px;margin-top:28px}.message{white-space:pre-wrap;overflow-wrap:anywhere;line-height:1.8}.image-link{display:grid;gap:8px;font-size:14px}.image-link img{display:block;max-width:100%;max-height:500px;object-fit:contain;border:1px solid #29404a}dl{display:grid;grid-template-columns:minmax(120px,35%) minmax(0,1fr);font-size:14px;line-height:1.7}dt,dd{margin:0;padding:7px 0;border-bottom:1px solid #20313a;overflow-wrap:anywhere}dd{padding-left:12px}dt{color:#a6b8c0}.empty{padding:28px;color:#a6b8c0}.pagination{display:flex;justify-content:space-between;gap:12px;padding:12px 0}.back{display:none}details{margin-top:24px}summary{cursor:pointer}footer{padding:20px 24px;color:#a6b8c0;font-size:12px;border-top:1px solid #29404a}
    @media(max-width:700px){header{padding:16px;flex-wrap:wrap}header small{display:block;margin-top:5px}form{padding:16px;gap:10px}label{flex:1;min-width:120px}label.version{flex-basis:100%}input{width:100%}main{display:block}.list{border-right:0}article{display:none;padding:20px}.viewing article{display:block}.viewing .list,.viewing form{display:none}.back{display:inline-block;margin-bottom:24px}dl{font-size:13px}.heading{gap:8px}}
  </style></head><body class="${explicitSelection ? 'viewing' : ''}"><header><div><h1>PHASE VECTOR / Feedback</h1><small>非公開 · 閲覧専用</small></div><a href="/cdn-cgi/access/logout">ログアウト</a></header>
  <form method="get" action="/"><label>カテゴリ<select name="category" aria-label="カテゴリ"><option value="">すべて</option>${categories.map(c => `<option value="${c}"${filters.category === c ? ' selected' : ''}>${labels[c]}</option>`).join('')}</select></label>
  <label>期間<select name="days" aria-label="期間">${['0', '7', '30', '90'].map(n => `<option value="${n}"${filters.days === n ? ' selected' : ''}>${n === '0' ? '全期間' : `${n} 日以内`}</option>`).join('')}</select></label>
  <label class="version">バージョン（完全一致）<input name="version" value="${escape(filters.version)}" maxlength="80" placeholder="例：2695e15"></label><button type="submit">絞り込む</button><a href="/">解除・最新へ</a></form>
  <main><nav class="list" aria-label="投稿一覧">${rows.length ? rows.map(row => {
    const p = JSON.parse(row.payload_json) as Payload;
    return `<a class="report" href="${link(filters, { id: row.id })}" aria-current="${selected?.id === row.id}"><div class="heading"><span class="tag">${escape(labels[p.category ?? ''] ?? p.category)}</span><time>${date(row.created_at)}</time></div><p class="excerpt">${escape(p.message)}</p><small>${escape(p.metadata.version)} · ${imageStatus(row)}</small></a>`;
  }).join('') : '<p class="empty">該当する投稿はありません。</p>'}<div class="pagination">${filters.before ? `<a href="${link(filters, { before: '' })}">最新へ</a>` : ''}${more ? `<a href="${link(filters, { before: `${rows.at(-1)!.created_at}.${rows.at(-1)!.id}` })}">以前の 25 件 →</a>` : ''}</div></nav><article>${detail}</article></main><footer>日時は UTC。画像には保存期限があります。投稿内容を公開しないでください。</footer></body></html>`;
}
