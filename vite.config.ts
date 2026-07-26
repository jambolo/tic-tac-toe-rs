/// <reference types="vitest/config" />
import { defineConfig } from "vitest/config";

// Vite config tuned for Tauri: fixed dev port, no screen clearing so Rust logs
// stay visible, and a build target modern webviews support. Imported from
// "vitest/config" rather than "vite" so the `test` block below is typed; this
// is otherwise a drop-in superset of a normal Vite config.
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
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts"],
  },
});
