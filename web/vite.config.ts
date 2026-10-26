import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  server: {
    // `cargo run` serves the API on 8080 during development.
    proxy: { "/api": "http://localhost:8080" },
  },
  // Recharts is most of the bundle; ~200 kB gzipped is fine for a LAN dashboard.
  build: { sourcemap: false, chunkSizeWarningLimit: 800 },
  test: { environment: "jsdom" },
});
