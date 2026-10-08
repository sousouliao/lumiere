import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { resolve } from 'node:path'

export default defineConfig({
  root: resolve(__dirname, 'src/renderer'),
  plugins: [react({}), tailwindcss()],
  resolve: { alias: { '@': resolve(__dirname, 'src/renderer/src') } },
  server: { host: '127.0.0.1', port: 5173, strictPort: true },
  build: { outDir: resolve(__dirname, 'out/renderer'), emptyOutDir: true },
})
