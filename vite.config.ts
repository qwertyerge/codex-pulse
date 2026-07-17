import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: { port: 5180, strictPort: true },
  build: { target: ["es2019", "safari13"], outDir: "dist", emptyOutDir: true },
  test: { environment: "jsdom", include: ["src/__tests__/**/*.spec.ts"] }
});
