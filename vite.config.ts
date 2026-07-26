import { defineConfig } from "vite";

// Vite config tuned for Tauri: fixed dev port, no screen clearing so Rust logs
// stay visible, and a build target modern webviews support.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    outDir: "dist",
    target: "esnext",
    emptyOutDir: true,
  },
});
