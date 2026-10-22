import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

const apiPort = process.env.API_PORT ?? '3001'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: Number(process.env.WEB_PORT ?? 5173),
    proxy: { '/api': `http://localhost:${apiPort}` }
  }
})
