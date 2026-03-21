import { defineConfig } from "vite";

const isDev = !!process.env.TAURI_DEBUG;

// https://vitejs.dev/config/
export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    watch: {
      // Don't trigger Vite reloads for Rust changes — Tauri handles that.
      ignored: ["**/src-tauri/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: ["es2021", "chrome105", "safari13"],
    minify: isDev ? false : "esbuild",
    sourcemap: isDev,
  },
});
