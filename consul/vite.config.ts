import tailwindcss from '@tailwindcss/vite';
import { defineConfig, loadEnv } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), '');
  const mem0Target = env.MEM0_API_URL?.replace(/\/$/, '');

  return {
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: {
        '@': path.resolve(__dirname, './src'),
      },
    },
    server: {
      port: 5173,
      proxy: mem0Target
        ? {
            '/mem0-api': {
              target: mem0Target,
              changeOrigin: true,
              rewrite: (p) => p.replace(/^\/mem0-api/, ''),
              configure: (proxy) => {
                proxy.on('proxyReq', (proxyReq) => {
                  if (env.MEM0_API_KEY) {
                    proxyReq.setHeader('X-API-Key', env.MEM0_API_KEY);
                  }
                });
              },
            },
          }
        : undefined,
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
  };
});
