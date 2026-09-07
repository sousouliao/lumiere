import react from '@vitejs/plugin-react'
import { resolve } from 'node:path'
import { defineConfig } from 'vitest/config'

export default defineConfig({
  plugins: [react({})],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src/renderer/src'),
    },
  },
  test: {
    environment: 'node',
    exclude: ['src/**/*.macos.test.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
  },
})
