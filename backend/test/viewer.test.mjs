import { before, after, test } from 'node:test';
import assert from 'node:assert/strict';
import { viewer, reportId, png } from './viewer-fixture.mjs';

let mf;
before(async () => { mf = await viewer(); });
after(async () => { await mf?.dispose(); });
const get = (path = '/') => mf.dispatchFetch(`https://viewer.test${path}`);

test('fails closed without configuration, trusted Access context, or an allowed email', async () => {
  for (const [options, expected] of [[{ allowed: '' }, 503], [{ access: false }, 403], [{ identity: {} }, 403], [{ identity: { email: 'intruder@example.test' } }, 403]]) {
    const denied = await viewer({ ...options, seed: false });
    try {
      for (const path of ['/', `/image/${reportId(29)}`]) {
        const response = await denied.dispatchFetch(`https://viewer.test${path}`, { headers: { 'Cf-Access-Authenticated-User-Email': 'viewer@example.test', 'Cf-Access-Jwt-Assertion': 'forged' } });
        assert.equal(response.status, expected); assert.match(response.headers.get('cache-control'), /no-store/);
      }
    } finally { await denied.dispose(); }
  }
});
test('escapes report text and serves a private server-rendered list/detail page', async () => {
  const response = await get(); assert.equal(response.status, 200);
  assert.match(response.headers.get('content-security-policy'), /frame-ancestors 'none'/);
  assert.equal(response.headers.get('referrer-policy'), 'no-referrer');
  assert.equal(response.headers.get('x-robots-tag'), 'noindex, nofollow');
  const html = await response.text();
  assert.match(html, /&lt;script&gt;alert\(&quot;private&quot;\)/); assert.doesNotMatch(html, /<script>/);
  assert.match(html, /<img src="\/image\//); assert.match(html, /以前の 25 件/);
  assert.doesNotMatch(html, new RegExp(`id=${reportId(1)}|id=${reportId(2)}`));
  assert.equal((await mf.dispatchFetch('https://viewer.test/', { method: 'HEAD' })).status, 200);
});
test('filters and keyset pagination retain complete feedback only, including tied timestamps', async () => {
  const html = await (await get()).text();
  const ids = body => [...body.matchAll(/class="report" href="[^"]*id=([a-f0-9-]+)/g)].map(m => m[1]);
  const next = html.match(/href="([^"]+)">以前の 25 件/)[1].replaceAll('&amp;', '&');
  const rest = await (await get(next)).text();
  assert.equal(ids(html).length, 25); assert.equal(ids(rest).length, 2);
  assert.equal(new Set([...ids(html), ...ids(rest)]).size, 27);
  const filtered = await (await get('/?category=balance&days=7&version=test-commit')).text();
  assert.equal(ids(filtered).length, 13);
  assert.doesNotMatch(filtered, /older-commit/);
  assert.match(await (await get('/?version=unknown')).text(), /該当する投稿はありません/);
  assert.equal((await get('/?version=%27%20OR%201%3D1--')).status, 200);
  for (const query of ['category=nope', 'days=-1', 'version=' + 'x'.repeat(81), 'before=bad', 'id=nope']) assert.equal((await get('/?' + query)).status, 400);
  for (const i of [1, 2, 999]) assert.equal((await get('/?id=' + reportId(i))).status, 404);
});
test('authenticated images are private, expiry is enforced before cleanup, and writes are rejected', async () => {
  const image = await get('/image/' + reportId(29));
  assert.equal(image.status, 200); assert.equal(image.headers.get('content-type'), 'image/png');
  assert.match(image.headers.get('cache-control'), /private, no-store/);
  assert.deepEqual(Buffer.from(await image.arrayBuffer()), png);
  assert.equal((await get('/image/' + reportId(28))).status, 410);
  assert.equal((await get('/image/' + reportId(27))).status, 404);
  assert.equal((await get('/image/invalid')).status, 404);
  for (const method of ['POST', 'PUT', 'PATCH', 'DELETE']) assert.equal((await mf.dispatchFetch('https://viewer.test/', { method })).status, 405);
  const bucket = await mf.getR2Bucket('SCREENSHOTS'); await bucket.delete(`screenshots/${reportId(29)}.png`);
  assert.equal((await get('/image/' + reportId(29))).status, 404);
  const db = await mf.getD1Database('DB'); assert.equal((await db.prepare('SELECT COUNT(*) AS n FROM submissions').first()).n, 29);
});
test('storage failures return a generic noncacheable response', async () => {
  const broken = await viewer({ seed: false });
  try { const response = await broken.dispatchFetch('https://viewer.test/'); assert.equal(response.status, 503); assert.doesNotMatch(await response.text(), /SELECT|D1_ERROR|submissions/); }
  finally { await broken.dispose(); }
});
