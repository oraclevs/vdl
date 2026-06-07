import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { viteSingleFile } from 'vite-plugin-singlefile'

// Builds to a single self-contained HTML file (JS + CSS inlined) so the Rust
// server can keep embedding it via `include_str!` — no separate asset pipeline.
export default defineConfig({
  plugins: [react(), tailwindcss(), viteSingleFile()],
  build: {
    outDir: 'dist',
    cssCodeSplit: false,
    assetsInlineLimit: 100000000,
  },
})
