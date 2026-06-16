import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// src-react/ is a self-contained frontend project: build configs live here and
// pnpm runs from this directory (Vite root defaults to this dir). Phase 2 wires
// Tauri to the dist/ produced here:
//   devUrl -> http://localhost:3000, frontendDist -> this dist/.
export default defineConfig({
  plugins: [react()],
  server: { port: 3000, strictPort: true },
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
