import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vitest/config";

// The frontend lives in ui/ and builds into dist/, which tauri.conf.json
// embeds. Tauri expects the dev server on a fixed port.
export default defineConfig({
  root: "ui",
  plugins: [vue()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { outDir: "../dist", emptyOutDir: true },
  test: {
    environment: "happy-dom",
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,vue}"],
      exclude: ["src/main.ts", "src/**/*.spec.ts"],
      reportsDirectory: "../coverage",
      thresholds: { lines: 100, functions: 100, branches: 100, statements: 100 },
    },
  },
});
