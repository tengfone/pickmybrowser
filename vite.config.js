import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";
import process from "node:process";

// Set by `tauri dev` when developing against a mobile device.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  // There is no Node server inside Tauri, so the app is built as a static SPA: every route
  // falls back to index.html.
  plugins: [sveltekit({ preprocess: vitePreprocess(), adapter: adapter({ fallback: "index.html" }) })],
  // Keep Rust compiler errors visible in the terminal.
  clearScreen: false,
  server: {
    // Tauri loads the dev server from a fixed port (devUrl in tauri.conf.json).
    port: 1420,
    strictPort: true,
    host: host || "127.0.0.1",
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**"] },
  },
}));
