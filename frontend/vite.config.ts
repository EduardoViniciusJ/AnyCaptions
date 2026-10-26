import { defineConfig } from "vitest/config";

export default defineConfig({
  clearScreen: false,
  server: { host: "127.0.0.1", port: 1420, strictPort: true },
  // assetsInlineLimit: 0 — um asset pequeno viraria `data:` URI embutida no JS,
  // e a CSP do overlay (img-src 'self') bloqueia `data:`; todo asset vira arquivo.
  build: { target: "es2022", assetsInlineLimit: 0 },
  test: { environment: "jsdom", include: ["src/**/*.test.ts"], restoreMocks: true },
});
