// Loopback browser fixture with simulated Access. Never deployed.
import { createServer } from 'node:http';
import { viewer } from './viewer-fixture.mjs';
const mf = await viewer();
const server = createServer(async (req, res) => {
  try {
    const response = await mf.dispatchFetch('https://viewer.test' + req.url, { method: req.method });
    res.statusCode = response.status;
    for (const [key, value] of response.headers) res.setHeader(key, value);
    res.end(Buffer.from(await response.arrayBuffer()));
  } catch { res.statusCode = 500; res.end('Fixture unavailable.'); }
});
server.listen(0, '127.0.0.1', () => console.log(`READY http://127.0.0.1:${server.address().port}`));
for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => server.close(async () => { await mf.dispose(); process.exit(0); }));
