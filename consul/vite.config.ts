/// <reference types="vitest/config" />
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
  /**
   * Pre-bundle heavy deps at dev-server start so first navigation does not
   * cold-transform three/recharts/dnd-kit on the main thread (multi-second hangs).
   */
  optimizeDeps: {
    include: [
      'three',
      '@react-three/fiber',
      '@react-three/drei',
      'recharts',
      '@tanstack/react-table',
      '@tanstack/react-virtual',
      '@tanstack/react-query',
      '@dnd-kit/core',
      '@dnd-kit/sortable',
      '@dnd-kit/utilities',
      '@dnd-kit/modifiers',
      'motion',
      '@bufbuild/protobuf',
    ],
  },
  server: {
    port: 5173,
    // Live telemetry: set VITE_CHAPPE_* in .env.local (see consul/.env.example).
    // WebTransport uses HTTP/3 to the gateway directly (not proxied through Vite).
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

          if (
            id.includes('three') ||
            id.includes('@react-three/fiber') ||
            id.includes('@react-three/drei')
          ) {
            return 'three';
          }
        },
      },
    },
  },
  test: {
    environment: 'jsdom',
  },
});
