import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// Frontend root is src-react/ (per ADR0001). Phase 2 wires Tauri to this:
//   devUrl -> http://localhost:3000, frontendDist -> ../src-react/dist
export default defineConfig({
  root: "src-react",
  plugins: [react()],
  server: { port: 3000, strictPort: true },
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src-react", import.meta.url)) },
  },
  build: { outDir: "dist", emptyOutDir: true },
});
