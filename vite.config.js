import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import process from "node:process";

// Set by `tauri dev` when developing against a mobile device.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  plugins: [sveltekit()],
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
