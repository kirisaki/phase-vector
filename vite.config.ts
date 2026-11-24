import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig, loadEnv } from 'vite';
import { execFileSync } from 'node:child_process';

const localPath = (path: string) => fileURLToPath(new URL(path, import.meta.url));
const web = localPath('./web/');
const pkg = localPath('./dist/pkg/');

export default defineConfig(({ mode }) => {
  const values = loadEnv(mode, process.cwd(), 'PV_');
  return {
  root: web,
  publicDir: false,
  resolve: {
    alias: [{ find: './pkg/phase_vector.js', replacement: `${pkg}phase_vector.js` }],
  },
  server: {
    host: '127.0.0.1',
    port: 5173,
    strictPort: true,
    fs: { allow: [web, pkg] },
  },
  plugins: [{
    name: 'require-built-wasm',
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        if (request.url?.split('?')[0] !== '/runtime-config.json') return next();
        response.setHeader('Content-Type', 'application/json');
        response.setHeader('Cache-Control', 'no-store');
        response.end(JSON.stringify({
          version: execFileSync('git', ['describe', '--always', '--dirty'], { encoding: 'utf8' }).trim(),
          feedbackApiUrl: values.PV_FEEDBACK_API_URL || '', turnstileSiteKey: values.PV_TURNSTILE_SITE_KEY || '',
        }));
      });
      for (const file of ['phase_vector.js', 'phase_vector_bg.wasm']) {
        if (!existsSync(`${pkg}${file}`)) {
          throw new Error('Build the Rust browser module first: npm run build:wasm');
        }
      }
    },
  }],
  };
});
