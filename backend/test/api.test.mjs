import { before, after, test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { deflateSync } from 'node:zlib';
import { Miniflare, convertV4MiniflareOptions, Response as WorkerResponse } from 'miniflare';

let mf, db, bucket;
const origin = 'https://kirisaki.github.io';
const secret = 'isolated-test-secret-never-a-production-credential';
async function outboundService(request) {
  assert.equal(request.url,'https://challenges.cloudflare.com/turnstile/v0/siteverify');
  const { response, secret: sent } = await request.json();
  assert.equal(sent,secret);
  if(response==='unavailable') return new WorkerResponse('{}',{status:500});
  return WorkerResponse.json({ success:response!=='invalid', hostname:response==='wrong-host'?'attacker.example':'kirisaki.github.io',
    action:response.startsWith('result:')?'result':'feedback' });
}
before(async () => {
  mf = new Miniflare(convertV4MiniflareOptions({ modules: true, scriptPath: 'dist/index.js', compatibilityDate: '2026-07-01',
    d1Databases: ['DB'], r2Buckets: ['SCREENSHOTS'], outboundService,
    bindings: { ENVIRONMENT: 'production', ALLOWED_ORIGINS: origin, TURNSTILE_HOSTNAMES: 'kirisaki.github.io',
      SCREENSHOT_RETENTION_DAYS: '30', TURNSTILE_SECRET_KEY: secret, RATE_LIMIT_SALT: 'test-only-salt-with-at-least-thirty-two-characters' } }));
  db = await mf.getD1Database('DB'); bucket = await mf.getR2Bucket('SCREENSHOTS');
  for (const sql of (await readFile('migrations/0001_feedback.sql', 'utf8')).split(';').filter(s => s.trim())) await db.prepare(sql).run();
});
after(async () => { await mf?.dispose();  });
function payload() {
  return { sessionId: crypto.randomUUID(), submissionId: crypto.randomUUID(), category: 'balance', message: 'A useful blockade trial.',
    metadata: { version: 'test-commit', outcome: 'running', runKind: 'standard', simulationSeconds: 120,
      forecastSeconds: 240, score: null, shotsFired: 4, ownDeltaVSpent: 1000, viewport: { width: 1360, height: 1000 } } };
}
function send(p = payload(), options = {}) {
  const { token = crypto.randomUUID(), path = '/v1/feedback', ip = crypto.randomUUID(), ...rest } = options;
  return mf.dispatchFetch(`https://api.example${path}`, { method: 'POST',
    headers: { origin, 'content-type': 'application/json', 'CF-Connecting-IP': ip },
    body: JSON.stringify({ payload: p, token }), ...rest });
}
function crc(bytes) { let n=0xffffffff; for(const b of bytes) { n^=b; for(let i=0;i<8;i++) n=(n>>>1)^(0xedb88320 & -(n&1)); } return (n^0xffffffff)>>>0; }
function chunk(type, data) { const b=Buffer.alloc(data.length+12); b.writeUInt32BE(data.length); b.write(type,4); data.copy(b,8); b.writeUInt32BE(crc(b.subarray(4,-4)),b.length-4); return b; }
function png(width=1,height=1) {
  const ihdr=Buffer.alloc(13); ihdr.writeUInt32BE(width); ihdr.writeUInt32BE(height,4); ihdr[8]=8; ihdr[9]=6;
  return Buffer.concat([Buffer.from([137,80,78,71,13,10,26,10]),chunk('IHDR',ihdr),chunk('IDAT',deflateSync(Buffer.from([0,255,0,0,255]))),chunk('IEND',Buffer.alloc(0))]);
}
async function upload(image, type='image/png') {
  const form = new FormData(); form.set('payload',JSON.stringify(payload())); form.set('token',crypto.randomUUID());
  form.set('screenshot',new Blob([image],{type}),'capture.png');
  const wire = new Request('https://api.example/v1/feedback',{method:'POST',body:form});
  return send(undefined,{ headers:{origin,'CF-Connecting-IP':crypto.randomUUID(),'content-type':wire.headers.get('content-type')}, body:await wire.arrayBuffer() });
}
test('stores a private submission and deduplicates retries without double writes', async () => {
  const p=payload(), first=await send(p); assert.equal(first.status,201);
  const {id}=await first.json(); const retry=await send(p,{token:'invalid'}); assert.equal(retry.status,200); assert.equal((await retry.json()).id,id);
  const row=await db.prepare('SELECT * FROM submissions WHERE id=?').bind(id).first();
  assert.equal(row.status,'complete'); assert.equal(row.image_key,null); assert.deepEqual(JSON.parse(row.payload_json),p);
  assert.equal((await send({...p,message:'changed'})).status,409);
  assert.equal((await mf.dispatchFetch(`https://api.example/v1/feedback/${id}`)).status,404);
  assert.equal((await mf.dispatchFetch('https://api.example/v1/feedback',{headers:{origin}})).status,405);
});
test('accepts cached clients with an optional legacy rating without requiring new reports to send one',async()=>{
  const legacy={...payload(),rating:4}; const response=await send(legacy);
  assert.equal(response.status,201);
  const {id}=await response.json();
  const row=await db.prepare('SELECT payload_json FROM submissions WHERE id=?').bind(id).first();
  assert.equal(JSON.parse(row.payload_json).rating,4);
  assert.equal((await send(legacy)).status,200);
});
test('strict metadata and legacy rating validation never stores enemy state or Lua',async()=>{
  for(const change of [{rating:0},{rating:6},{rating:2.5},{category:'unknown'},{message:''},{message:'x'.repeat(4001)},
    {sessionId:'not-a-uuid'},{metadata:{...payload().metadata,enemy:{remaining_dv:1}}},{lua:'secret'},
    {metadata:{...payload().metadata,simulationSeconds:-1}},{metadata:{...payload().metadata,score:99}}]) {
    assert.equal((await send({...payload(),...change})).status,400);
  }
});
test('checks Turnstile success, hostname, action and local replay claims',async()=>{
  for(const token of ['invalid','wrong-host','result:wrong-action']) assert.equal((await send(payload(),{token})).status,403);
  assert.equal((await send(payload(),{token:'unavailable'})).status,503);
  const token=crypto.randomUUID(); assert.equal((await send(payload(),{token})).status,201);
  assert.equal((await send(payload(),{token})).status,403);
});
test('supports opt-in terminal results without comments or images',async()=>{
  const p=payload(); delete p.rating; delete p.message; delete p.category;
  assert.equal((await send(p,{path:'/v1/results',token:'result:'+crypto.randomUUID()})).status,400);
  p.metadata.outcome='timeout'; p.metadata.score=1807;
  assert.equal((await send(p,{path:'/v1/results',token:'result:'+crypto.randomUUID()})).status,201);
});
test('validates and privately stores PNG with expiration, rejects disguised or oversized images',async()=>{
  const res=await upload(png()); assert.equal(res.status,201,await res.clone().text());
  const {id}=await res.json(); const row=await db.prepare('SELECT * FROM submissions WHERE id=?').bind(id).first();
  assert.equal(row.image_expires_at-row.created_at,30*86400); assert.ok(await bucket.head(row.image_key));
  assert.equal((await mf.dispatchFetch('https://api.example/'+row.image_key)).status,404);
  for(const [image,type,status] of [[png(),'image/svg+xml',415],[Buffer.from('<svg/>'),'image/png',415],
    [png(5000,1),'image/png',415],[png(2,2),'image/png',415],[png().subarray(0,40),'image/png',415],
    [new Uint8Array(2*1024*1024+1),'image/png',413]]) assert.equal((await upload(image,type)).status,status);
  await db.prepare('UPDATE submissions SET image_expires_at=0 WHERE id=?').bind(id).run();
  const worker=await mf.getWorker(); await worker.scheduled({scheduledTime:Date.now(),cron:'17 * * * *'});
  assert.equal(await bucket.head(row.image_key),null);
  assert.equal((await db.prepare('SELECT image_key FROM submissions WHERE id=?').bind(id).first()).image_key,null);
});
test('limits sessions and IPs atomically, including concurrent submissions',async()=>{
  const p=payload(); const responses=await Promise.all(Array.from({length:8},()=>send({...p,submissionId:crypto.randomUUID()})));
  assert.equal(responses.filter(r=>r.status===201).length,5); assert.equal(responses.filter(r=>r.status===429).length,3);
  const ip=crypto.randomUUID(); for(let i=0;i<20;i++) assert.equal((await send(payload(),{ip})).status,201);
  const denied=await send(payload(),{ip}); assert.equal(denied.status,429); assert.equal(denied.headers.get('retry-after'),'600');
});
test('CORS is exact, bodies are bounded, malformed requests return generic errors',async()=>{
  const bad=await send(payload(),{headers:{origin:'https://kirisaki.github.io.attacker.example','content-type':'application/json'}});
  assert.equal(bad.status,403); assert.equal(bad.headers.get('access-control-allow-origin'),null);
  const preflight=await mf.dispatchFetch('https://api.example/v1/feedback',{method:'OPTIONS',headers:{origin}});
  assert.equal(preflight.status,204); assert.equal(preflight.headers.get('access-control-allow-origin'),origin);
  assert.equal((await send(undefined,{body:'{'})).status,400);
  assert.equal((await send(undefined,{body:'x'.repeat(2*1024*1024+25000)})).status,413);
});
test('storage failure does not leak request data or a stack trace',async()=>{
  await db.prepare('DROP TABLE submissions').run();
  const res=await send(); assert.equal(res.status,503); assert.deepEqual(await res.json(),{error:'service_unavailable'});
});
