import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { fileURLToPath } from "node:url";

const host = (globalThis as { process?: { env: Record<string, string | undefined> } }).process?.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    target: "es2022",
    rollupOptions: {
      input: {
        main: fileURLToPath(new URL("./index.html", import.meta.url)),
        overlay: fileURLToPath(new URL("./overlay.html", import.meta.url)),
      },
    },
  },
});
