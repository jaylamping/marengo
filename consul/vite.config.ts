import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    port: 5173,
    // Future: proxy or WebTransport setup will go here
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (
            id.includes('node_modules/recharts') ||
            id.includes('node_modules/d3-') ||
            id.includes('node_modules/decimal.js-light') ||
            id.includes('node_modules/es-toolkit')
          ) {
            return 'recharts';
          }

          if (
            id.includes('@tanstack/react-virtual') ||
            id.includes('@dnd-kit') ||
            id.includes('@tanstack/react-table') ||
            id.includes('@tanstack/table-core')
          ) {
            return 'inventory-table';
          }
        },
      },
    },
  },
});
