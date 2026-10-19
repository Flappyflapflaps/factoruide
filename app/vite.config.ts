import react from '@vitejs/plugin-react'
import { readFileSync } from 'node:fs'
import { defineConfig, type Plugin } from 'vite'

// The web build (`vite build --mode web`, see scripts/build-web.sh): the Tauri modules are
// replaced by the backend in Web Workers (src/web), and the page must be cross-origin isolated
// for the backend's threads (shared memory).
const web = {
  '@tauri-apps/api/core': '/src/web/shims/core.ts',
  '@tauri-apps/api/event': '/src/web/shims/event.ts',
  '@tauri-apps/api/app': '/src/web/shims/app.ts',
  '@tauri-apps/plugin-dialog': '/src/web/shims/dialog.ts',
  '@tauri-apps/plugin-updater': '/src/web/shims/updater.ts',
  '@tauri-apps/plugin-process': '/src/web/shims/process.ts',
}

const isolation = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
}

/** The app's version: the release's (`FG_VERSION`, set by scripts/build-web.sh), else the app's Cargo.toml. */
function appVersion(): string {
  const toml = readFileSync(new URL('./src-tauri/Cargo.toml', import.meta.url), 'utf8')
  return process.env.FG_VERSION ?? toml.match(/^version = "([^"]+)"/m)?.[1] ?? '0.0.0'
}

/** coi.js next to index.html, loaded before anything else (see src/web/coi.js). */
const isolationWorker = (): Plugin => ({
  name: 'fg-coi',
  apply: 'build',
  transformIndexHtml: (html) => html.replace('<head>', '<head>\n    <script src="./coi.js"></script>'),
  generateBundle() {
    this.emitFile({ type: 'asset', fileName: 'coi.js', source: readFileSync(new URL('./src/web/coi.js', import.meta.url), 'utf8') })
  },
})

// https://vite.dev/config/
export default defineConfig(({ mode }) => ({
  plugins: mode === 'web' ? [react(), isolationWorker()] : [react()],
  // Relative paths: the site works from any folder of a server.
  base: mode === 'web' ? './' : '/',
  resolve: { alias: mode === 'web' ? web : {} },
  define: { __APP_VERSION__: JSON.stringify(appVersion()) },
  worker: { format: 'es' },
  build: mode === 'web' ? { outDir: process.env.FG_WEB_OUT ?? '../web/dist', emptyOutDir: true, target: 'es2023' } : {},
  // The changelog shown in the app is the project's, one level up.
  server: { fs: { allow: ['..'] }, headers: mode === 'web' ? isolation : {} },
  preview: { headers: isolation },
}))
