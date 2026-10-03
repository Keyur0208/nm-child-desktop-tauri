import { defineConfig } from 'vite';

export default defineConfig({
    clearScreen: false,
    server: {
        port: 8081,
        strictPort: true,
    },
    envPrefix: ['VITE_', 'TAURI_', 'APP_', 'API_', 'LOG_'],
    build: {
        target: 'chrome105',
        minify: 'esbuild',
        sourcemap: false,
        outDir: 'dist',
        emptyOutDir: true,
    },
});
