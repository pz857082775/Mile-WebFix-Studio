import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  publicDir: 'public',
  build: { outDir: 'dist', emptyOutDir: true },
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://127.0.0.1:8787',
      '/proxy': 'http://127.0.0.1:8787',
      '/demo': 'http://127.0.0.1:8787',
      '/sitemap.xml': 'http://127.0.0.1:8787',
    },
  },
});