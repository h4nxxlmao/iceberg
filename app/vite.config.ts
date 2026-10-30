import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { resolve } from "node:path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  resolve: {
    alias: {
      $shared: resolve("src/shared"),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "chrome110",
    cssCodeSplit: true,
    reportCompressedSize: true,
    rollupOptions: {
      input: {
        widget: resolve("index.html"),
        settings: resolve("settings.html"),
      },
      output: {
        manualChunks: undefined,
      },
    },
  },
});
