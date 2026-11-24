import { categories } from '../src/validation';
import { renderPage, type Report, type Filters } from './page';

interface Env { DB: D1Database; SCREENSHOTS: R2Bucket; VIEWER_EMAILS?: string }
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const completeFeedback = "status='complete' AND kind='feedback'";
const pageSize = 25;

function headers(nonce: string): HeadersInit {
  return {
    'cache-control': 'private, no-store', 'x-content-type-options': 'nosniff',
    'referrer-policy': 'no-referrer', 'x-robots-tag': 'noindex, nofollow',
    'content-security-policy': `default-src 'none'; style-src 'nonce-${nonce}'; img-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'`,
  };
}

export default {
  async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    const nonce = crypto.randomUUID();
    const reply = (body: BodyInit | null, status = 200, extra: HeadersInit = {}) => new Response(
      request.method === 'HEAD' ? null : body, { status, headers: { ...headers(nonce), ...extra } });
    try {
      const allowed = (env.VIEWER_EMAILS ?? '').split(',').map(s => s.trim().toLowerCase()).filter(Boolean);
      if (!allowed.length) return reply('Viewer is not configured.', 503);
      // Trust only the runtime-authenticated Access context, never client headers.
      const identity = await ctx.access?.getIdentity();
      if (!identity?.email || !allowed.includes(identity.email.toLowerCase())) return reply('Access denied.', 403);
      if (!['GET', 'HEAD'].includes(request.method)) return reply('Read-only viewer.', 405, { allow: 'GET, HEAD' });
      const url = new URL(request.url);
      if (url.pathname.startsWith('/image/')) {
        const id = url.pathname.slice('/image/'.length);
        if (!uuid.test(id)) return reply('Image not found.', 404);
        const row = await env.DB.prepare(`SELECT image_key,image_expires_at FROM submissions WHERE ${completeFeedback} AND id=?`).bind(id)
          .first<{ image_key: string | null; image_expires_at: number | null }>();
        if (row?.image_expires_at != null && row.image_expires_at <= Date.now() / 1000) return reply('Image expired.', 410);
        if (row?.image_key !== `screenshots/${id}.png`) return reply('Image not found.', 404);
        const image = await env.SCREENSHOTS.get(row.image_key);
        if (!image) return reply('Image not found.', 404);
        return reply(image.body, 200, { 'content-type': 'image/png', 'content-disposition': 'inline; filename="feedback.png"' });
      }
      if (url.pathname !== '/') return reply('Not found.', 404);
      const p = url.searchParams;
      const filters: Filters = { category: p.get('category') ?? '', days: p.get('days') ?? '0', version: (p.get('version') ?? '').trim(), before: p.get('before') ?? '' };
      const id = p.get('id');
      const cursor = filters.before ? /^(\d{1,12})\.([0-9a-f-]{36})$/.exec(filters.before) : null;
      if ((filters.category && !categories.includes(filters.category as typeof categories[number]))
        || !['0', '7', '30', '90'].includes(filters.days) || filters.version.length > 80
        || (id !== null && !uuid.test(id)) || (filters.before && (!cursor || !uuid.test(cursor[2])))) return reply('Invalid filter.', 400);
      const clauses = [completeFeedback], bindings: (string | number)[] = [];
      if (filters.category) { clauses.push("json_extract(payload_json,'$.category')=?"); bindings.push(filters.category); }
      if (filters.days !== '0') { clauses.push('created_at>=?'); bindings.push(Math.floor(Date.now() / 1000) - Number(filters.days) * 86400); }
      if (filters.version) { clauses.push("json_extract(payload_json,'$.metadata.version')=?"); bindings.push(filters.version); }
      if (cursor) { clauses.push('(created_at<? OR (created_at=? AND id<?))'); bindings.push(Number(cursor[1]), Number(cursor[1]), cursor[2]); }
      const result = await env.DB.prepare(`SELECT id,created_at,payload_json,image_key,image_expires_at FROM submissions WHERE ${clauses.join(' AND ')} ORDER BY created_at DESC,id DESC LIMIT ?`)
        .bind(...bindings, pageSize + 1).all<Report>();
      const rows = result.results.slice(0, pageSize);
      const selected = id ? await env.DB.prepare(`SELECT id,created_at,payload_json,image_key,image_expires_at FROM submissions WHERE ${completeFeedback} AND id=?`).bind(id).first<Report>() : rows[0] ?? null;
      if (id && !selected) return reply('Report not found.', 404);
      return reply(renderPage({ rows, selected, filters, more: result.results.length > pageSize, explicitSelection: !!id, nonce }), 200,
        { 'content-type': 'text/html; charset=utf-8' });
    } catch {
      // Do not log private reports, identities, or storage errors.
      return reply('Viewer temporarily unavailable. Please retry.', 503);
    }
  },
} satisfies ExportedHandler<Env>;
