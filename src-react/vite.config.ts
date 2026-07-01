import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";

// src-react/ is a self-contained frontend project: build configs live here and
// pnpm runs from this directory (Vite root defaults to this dir). Phase 2 wires
// Tauri to the dist/ produced here:
//   devUrl -> http://localhost:3000, frontendDist -> this dist/.
export default defineConfig({
  plugins: [react()],
  server: { port: 3001, strictPort: true },
  resolve: {
    alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (!id.includes("node_modules")) return;
          if (id.includes("@tauri-apps/")) return "tauri";
          if (id.includes("react-markdown") || id.includes("remark-")) return "markdown";
          if (id.includes("@dnd-kit/")) return "dnd";
          if (id.includes("lucide-react")) return "icons";
          return "vendor";
        },
      },
    },
  },
});
