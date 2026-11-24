// Local test harness only. This file is never bundled into the deployed Worker.
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { Miniflare, convertV4MiniflareOptions, Response as WorkerResponse } from 'miniflare';
import { fileURLToPath } from 'node:url';
const root = new URL('../',import.meta.url);
const mf = new Miniflare(convertV4MiniflareOptions({ modules:true, scriptPath:fileURLToPath(new URL('dist/index.js',root)), compatibilityDate:'2026-07-01',
  d1Databases:['DB'],r2Buckets:['SCREENSHOTS'], bindings:{ ENVIRONMENT:'production',ALLOWED_ORIGINS:'https://phase-vector.test',
    TURNSTILE_HOSTNAMES:'phase-vector.test',SCREENSHOT_RETENTION_DAYS:'30',TURNSTILE_SECRET_KEY:'browser-test-secret-only',
    RATE_LIMIT_SALT:'browser-tests-only-random-looking-private-salt' },
  outboundService:async request=>{
    if(request.url!=='https://challenges.cloudflare.com/turnstile/v0/siteverify') throw new Error('Unexpected external request');
    const body=await request.json();
    return WorkerResponse.json({success:body.response.startsWith('browser:'),hostname:'phase-vector.test',action:'feedback'});
  },
}));
const db=await mf.getD1Database('DB'),bucket=await mf.getR2Bucket('SCREENSHOTS');
for(const sql of (await readFile(new URL('migrations/0001_feedback.sql',root),'utf8')).split(';').filter(s=>s.trim())) await db.prepare(sql).run();
let loseNext=false;
const server=createServer(async(req,res)=>{
  try {
    if(req.url==='/__test/lose-next') {loseNext=true;res.end('ok');return;}
    if(req.url==='/__test/submissions') {res.setHeader('content-type','application/json');res.end(JSON.stringify((await db.prepare("SELECT * FROM submissions WHERE status='complete' ORDER BY created_at,id").all()).results));return;}
    if(req.url.startsWith('/__test/image/')) {
      const row=await db.prepare('SELECT image_key FROM submissions WHERE id=?').bind(req.url.split('/').at(-1)).first();
      const image=row?.image_key?await bucket.get(row.image_key):null;
      res.statusCode=image?200:404;res.setHeader('content-type','image/png');res.end(image?Buffer.from(await image.arrayBuffer()):'');return;
    }
    const chunks=[];for await(const chunk of req)chunks.push(chunk);
    const headers={...req.headers,'CF-Connecting-IP':'192.0.2.1'};delete headers.host;
    const response=await mf.dispatchFetch('https://feedback.test'+req.url,{method:req.method,headers,body:['GET','HEAD'].includes(req.method)?undefined:Buffer.concat(chunks)});
    for(const [key,value]of response.headers)res.setHeader(key,value);
    if(loseNext&&response.status===201){loseNext=false;res.statusCode=503;res.end('{"error":"simulated_lost_acknowledgment"}');return;}
    res.statusCode=response.status;res.end(Buffer.from(await response.arrayBuffer()));
  }catch{res.statusCode=500;res.end('test harness error');}
});
server.listen(0,'127.0.0.1',()=>console.log(`READY http://127.0.0.1:${server.address().port}`));
for(const signal of ['SIGINT','SIGTERM'])process.on(signal,()=>server.close(async()=>{await mf.dispose();process.exit(0);}));
