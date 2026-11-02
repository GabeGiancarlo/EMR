import { defineConfig } from "vite";
const apiTarget = process.env.NEXUS_API_PROXY || "http://127.0.0.1:8090";
export default defineConfig({
  server: {
    port: 5173,
    strictPort: true,
    proxy: { "/api": apiTarget, "/healthz": apiTarget },
  },
  preview: { port: 4173 },
  build: { target: "es2022", sourcemap: false },
});
