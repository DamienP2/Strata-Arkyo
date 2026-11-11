import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// En développement, l'API du moteur (strata serve) répond sur le port 8090.
export default defineConfig({
  plugins: [svelte()],
  build: { outDir: 'dist', emptyOutDir: true, assetsInlineLimit: 0, sourcemap: false },
  server: { proxy: { '/api': 'http://127.0.0.1:8090' } },
});
