import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:4096",
        changeOrigin: true,
        ws: true
      }
    }
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: false,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("monaco-editor")) return "monaco";
          if (id.includes("mermaid")) return "mermaid";
          if (id.includes("@xterm")) return "terminal";
          if (id.includes("react") || id.includes("scheduler")) return "react";
          return undefined;
        }
      }
    }
  }
});
