import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { readFileSync } from 'fs'

const host = process.env.TAURI_DEV_HOST
const { version } = JSON.parse(readFileSync('./package.json', 'utf-8'))

// The release's name, as its tag spells it (without the leading `v`). A
// pre-genesis version carries its sub-version as semver build metadata
// (`3.1.0+alpha.10`) so every package format accepts it; the tag keeps the
// pre-release form (`v3.1.0-alpha.10`) that shekyl-core's genesis gate
// requires. A version without build metadata is its own release name.
const release = version.replace('+', '-')

export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: {
    __APP_RELEASE__: JSON.stringify(release),
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: 'ws', host, port: 1421 }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
})
