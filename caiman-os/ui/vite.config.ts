import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { '@': path.resolve(__dirname, './src') },
  },
  server: {
    port: 3000,
    proxy: {
      '/api': { target: 'http://localhost:8765', changeOrigin: true },
      '/socket.io': { target: 'http://localhost:8765', ws: true },
    },
  },
  build: {
    outDir: 'dist',
    rollupOptions: {
      output: {
        manualChunks: {
          vendor:    ['react', 'react-dom', 'react-router-dom'],
          query:     ['@tanstack/react-query'],
          motion:    ['framer-motion'],
          flow:      ['@xyflow/react'],
          charts:    ['recharts'],
          editor:    ['@monaco-editor/react'],
          terminal:  ['@xterm/xterm'],
        },
      },
    },
  },
})
