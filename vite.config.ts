import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

const localPath = (path: string) => fileURLToPath(new URL(path, import.meta.url));
const web = localPath('./web/');
const pkg = localPath('./dist/pkg/');

export default defineConfig({
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
    configureServer() {
      for (const file of ['phase_vector.js', 'phase_vector_bg.wasm']) {
        if (!existsSync(`${pkg}${file}`)) {
          throw new Error('Build the Rust browser module first: npm run build:wasm');
        }
      }
    },
  }],
});
