import { ApiError, boundedBody, validateImage, validatePayload, type Kind } from './validation';

export interface Env {
  DB: D1Database;
  SCREENSHOTS: R2Bucket;
  ENVIRONMENT: string;
  ALLOWED_ORIGINS: string;
  TURNSTILE_HOSTNAMES: string;
  SCREENSHOT_RETENTION_DAYS: string;
  TURNSTILE_SECRET_KEY: string;
  RATE_LIMIT_SALT: string;
}
interface Stored { id: string; fingerprint: string; status: string; image_key: string | null }
const split = (s: string) => s.split(',').map(v => v.trim()).filter(Boolean);
async function hash(data: string | ArrayBuffer): Promise<string> {
  const digest = await crypto.subtle.digest('SHA-256', typeof data === 'string' ? new TextEncoder().encode(data) : data);
  return [...new Uint8Array(digest)].map(b => b.toString(16).padStart(2, '0')).join('');
}
function configured(env: Env) {
  const days = Number(env.SCREENSHOT_RETENTION_DAYS);
  if (!env.TURNSTILE_SECRET_KEY || !env.RATE_LIMIT_SALT || env.RATE_LIMIT_SALT.length < 32
    || !Number.isInteger(days) || days < 1 || days > 365
    || !env.ALLOWED_ORIGINS || !env.TURNSTILE_HOSTNAMES
    || (env.ENVIRONMENT !== 'development' && /^\d+x0{10}/.test(env.TURNSTILE_SECRET_KEY))) throw new ApiError(503, 'not_configured');
}
async function limit(env: Env, key: string, maximum: number, now: number) {
  const row = await env.DB.prepare(`INSERT INTO rate_limits(key,count,expires_at) VALUES (?,1,?)
    ON CONFLICT(key) DO UPDATE SET count = count + 1 RETURNING count`).bind(key, now + 600).first<{ count: number }>();
  if (!row || row.count > maximum) throw new ApiError(429, 'rate_limited');
}
async function verify(env: Env, token: string, hostname: string, kind: Kind) {
  let result: { success?: boolean; hostname?: string; action?: string };
  try {
    const response = await fetch('https://challenges.cloudflare.com/turnstile/v0/siteverify', {
      method: 'POST', headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ secret: env.TURNSTILE_SECRET_KEY, response: token }),
      signal: AbortSignal.timeout(8000),
    });
    if (!response.ok) throw new Error('unavailable');
    result = await response.json();
  } catch { throw new ApiError(503, 'verification_unavailable'); }
  // Official dummy keys return a dummy hostname/action and are local-only.
  const dummy = env.ENVIRONMENT === 'development' && env.TURNSTILE_SECRET_KEY === '1x0000000000000000000000000000000AA';
  if (!result.success || (!dummy && (result.hostname !== hostname || result.action !== kind))) throw new ApiError(403, 'verification_failed');
  if (dummy) return;
  // Siteverify token reuse detection can converge asynchronously; claim locally too.
  const claimed = await env.DB.prepare('INSERT OR IGNORE INTO rate_limits(key,count,expires_at) VALUES (?,1,?)')
    .bind(`token:${await hash(token)}`, Math.floor(Date.now() / 1000) + 600).run();
  if (!claimed.meta.changes) throw new ApiError(403, 'verification_failed');
}
async function submit(request: Request, env: Env, kind: Kind, origin: string) {
  configured(env);
  const now = Math.floor(Date.now() / 1000), window = Math.floor(now / 600);
  const ip = request.headers.get('CF-Connecting-IP') || (env.ENVIRONMENT === 'development' ? 'local' : '');
  if (!ip) throw new ApiError(403, 'missing_client_address');
  const ipKey = await hash(`${env.RATE_LIMIT_SALT}:${window}:${ip}`);
  await limit(env, `ip:${window}:${ipKey}`, 20, now);
  const body = await boundedBody(request);
  let data: unknown, token: unknown, image: ArrayBuffer | null = null;
  try {
    const type = request.headers.get('content-type') || '';
    if (type.startsWith('multipart/form-data;')) {
      const form = await new Response(body, { headers: { 'content-type': type } }).formData();
      const keys = [...form.keys()];
      if (keys.some(k => !['payload', 'token', 'screenshot'].includes(k)) || new Set(keys).size !== keys.length) throw new ApiError(400, 'invalid_fields');
      const payload = form.get('payload');
      if (typeof payload !== 'string' || payload.length > 16000) throw new ApiError(400, 'invalid_payload');
      data = JSON.parse(payload); token = form.get('token');
      const file = form.get('screenshot');
      if (file !== null) {
        if (kind !== 'feedback' || typeof file === 'string') throw new ApiError(400, 'invalid_image');
        image = await validateImage(file);
      }
    } else if (type.split(';')[0] === 'application/json') {
      if (body.byteLength > 24000) throw new ApiError(413, 'request_too_large');
      const envelope = JSON.parse(new TextDecoder().decode(body));
      if (!envelope || typeof envelope !== 'object' || Object.keys(envelope).some(k => !['payload', 'token'].includes(k))) throw new ApiError(400, 'invalid_fields');
      data = envelope.payload; token = envelope.token;
    } else throw new ApiError(415, 'unsupported_content_type');
  } catch (e) { if (e instanceof ApiError) throw e; throw new ApiError(400, 'invalid_payload'); }
  const payload = validatePayload(data, kind);
  if (typeof token !== 'string' || !token || token.length > 2048) throw new ApiError(400, 'invalid_token');
  await limit(env, `session:${window}:${payload.sessionId}`, 5, now);
  const fingerprint = await hash(JSON.stringify({ kind, payload, image: image ? await hash(image) : null }));
  const lookup = () => env.DB.prepare('SELECT id,fingerprint,status,image_key FROM submissions WHERE session_id = ? AND submission_id = ?')
    .bind(payload.sessionId, payload.submissionId).first<Stored>();
  const existing = await lookup();
  if (existing) {
    if (existing.fingerprint !== fingerprint) throw new ApiError(409, 'id_conflict');
    if (existing.status !== 'complete') throw new ApiError(409, 'submission_pending');
    return { id: existing.id, duplicate: true };
  }
  const hostname = new URL(origin).hostname;
  if (!split(env.TURNSTILE_HOSTNAMES).includes(hostname)) throw new ApiError(403, 'origin_forbidden');
  await verify(env, token, hostname, kind);
  const id = crypto.randomUUID(), imageKey = image ? `screenshots/${id}.png` : null;
  const inserted = await env.DB.prepare(`INSERT OR IGNORE INTO submissions
    (id,session_id,submission_id,kind,fingerprint,status,created_at,payload_json,image_key,image_expires_at)
    VALUES (?,?,?,?,?,'pending',?,?,?,?)`).bind(id, payload.sessionId, payload.submissionId, kind, fingerprint, now,
      JSON.stringify(payload), imageKey, image ? now + Number(env.SCREENSHOT_RETENTION_DAYS) * 86400 : null).run();
  if (!inserted.meta.changes) throw new ApiError(409, 'submission_pending');
  try {
    if (image && imageKey) await env.SCREENSHOTS.put(imageKey, image, { httpMetadata: { contentType: 'image/png', cacheControl: 'private, no-store' } });
    await env.DB.prepare("UPDATE submissions SET status = 'complete' WHERE id = ?").bind(id).run();
  } catch {
    // D1 and R2 are not one transaction. Preserve an uncertain completed write;
    // otherwise compensate, with scheduled cleanup as the crash-recovery fallback.
    const stored = await lookup();
    if (stored?.status === 'complete') return { id, duplicate: true };
    if (imageKey) await env.SCREENSHOTS.delete(imageKey);
    await env.DB.prepare("DELETE FROM submissions WHERE id = ? AND status = 'pending'").bind(id).run();
    throw new ApiError(503, 'storage_unavailable');
  }
  return { id, duplicate: false };
}
export async function cleanup(env: Env, now = Math.floor(Date.now() / 1000)) {
  const rows = await env.DB.prepare(`SELECT id,image_key,status FROM submissions WHERE
    (image_key IS NOT NULL AND image_expires_at <= ?) OR (status = 'pending' AND created_at <= ?) LIMIT 20`)
    .bind(now, now - 3600).all<Stored>();
  for (const row of rows.results) {
    if (row.image_key) await env.SCREENSHOTS.delete(row.image_key);
    if (row.status === 'pending') await env.DB.prepare("DELETE FROM submissions WHERE id = ? AND status = 'pending'").bind(row.id).run();
    else await env.DB.prepare('UPDATE submissions SET image_key = NULL WHERE id = ?').bind(row.id).run();
  }
  await env.DB.prepare('DELETE FROM rate_limits WHERE expires_at <= ?').bind(now).run();
}
export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const origin = request.headers.get('origin') || '';
    const headers = new Headers({ 'content-type': 'application/json', 'cache-control': 'no-store', 'vary': 'Origin', 'x-content-type-options': 'nosniff' });
    const allowed = split(env.ALLOWED_ORIGINS || '').includes(origin);
    if (allowed) headers.set('access-control-allow-origin', origin);
    const reply = (status: number, data: unknown) => new Response(JSON.stringify(data), { status, headers });
    try {
      const path = new URL(request.url).pathname;
      if (path === '/health' && request.method === 'GET') return reply(200, { ok: true });
      if (!['/v1/feedback', '/v1/results'].includes(path)) throw new ApiError(404, 'not_found');
      if (!allowed) throw new ApiError(403, 'origin_forbidden');
      if (request.method === 'OPTIONS') {
        headers.set('access-control-allow-methods', 'POST');
        headers.set('access-control-allow-headers', 'Content-Type');
        headers.set('access-control-max-age', '600');
        return new Response(null, { status: 204, headers });
      }
      if (request.method !== 'POST') throw new ApiError(405, 'method_not_allowed');
      const result = await submit(request, env, path.endsWith('feedback') ? 'feedback' : 'result', origin);
      return reply(result.duplicate ? 200 : 201, result);
    } catch (error) {
      const known = error instanceof ApiError ? error : new ApiError(503, 'service_unavailable');
      if (known.status === 429) headers.set('retry-after', '600');
      if (known.status === 503 || known.code === 'submission_pending') headers.set('retry-after', '5');
      return reply(known.status, { error: known.code });
    }
  },
  async scheduled(_controller: ScheduledController, env: Env): Promise<void> { await cleanup(env); },
} satisfies ExportedHandler<Env>;
