// Test-only data and Access simulation; never imported by the deployed Worker.
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { Miniflare, convertV4MiniflareOptions } from 'miniflare';

export const email = 'viewer@example.test';
export const reportId = n => `00000000-0000-4000-8000-${String(n).padStart(12, '0')}`;
export const png = await readFile(new URL('../../web/assets/ogp-start.png', import.meta.url));
export async function viewer({ identity = { email }, allowed = email, access = true, seed = true } = {}) {
  const options = convertV4MiniflareOptions({ modules: true,
    scriptPath: fileURLToPath(new URL('../dist/viewer/index.js', import.meta.url)), compatibilityDate: '2026-11-24',
    d1Databases: ['DB'], r2Buckets: ['SCREENSHOTS'], bindings: { VIEWER_EMAILS: allowed } });
  if (access) options.workers[0].config.access = { dev: { aud: 'viewer-local', identity } };
  const mf = new Miniflare(options);
  if (seed) {
    const db = await mf.getD1Database('DB'), bucket = await mf.getR2Bucket('SCREENSHOTS');
    for (const sql of (await readFile(new URL('../migrations/0001_feedback.sql', import.meta.url), 'utf8')).split(';').filter(s => s.trim())) await db.prepare(sql).run();
    const now = Math.floor(Date.now() / 1000);
    for (let i = 1; i <= 29; i++) {
      const id = reportId(i);
      const payload = { sessionId: reportId(999), submissionId: id, category: i % 2 ? 'balance' : 'controls',
        message: i === 29 ? '<script>alert("private")</script> 敵の回避について\n機動と射撃を組み合わせると楽しめました。' : `投稿 ${i}：${'操作とバランスの確認。'.repeat(10)}`,
        metadata: { version: i % 2 ? 'test-commit' : 'older-commit', outcome: 'running', runKind: 'standard', simulationSeconds: 120,
          forecastSeconds: 240, score: null, shotsFired: 4, ownDeltaVSpent: 1000, viewport: { width: 1360, height: 1000 } } };
      await db.prepare('INSERT INTO submissions VALUES (?,?,?,?,?,?,?,?,?,?)').bind(id, payload.sessionId, id, i === 1 ? 'result' : 'feedback', 'fixture',
        i === 2 ? 'pending' : 'complete', i === 3 ? now - 100 * 86400 : now, JSON.stringify(payload), i >= 28 ? `screenshots/${id}.png` : null,
        i >= 28 ? now + (i === 28 ? -60 : 86400) : null).run();
      if (i >= 28) await bucket.put(`screenshots/${id}.png`, png);
    }
  }
  return mf;
}
