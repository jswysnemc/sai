import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
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
          // 语言高亮包由 language-data 按需异步加载，交给 Rollup 自动拆分，
          // 并入核心分包会让全部语言随编辑器一次性加载
          if (/@codemirror\/(lang-(?!markdown|html)|legacy-modes)/.test(id)) return undefined;
          if (/@lezer\/(?!common|highlight|lr|markdown)/.test(id)) return undefined;
          if (id.includes("@codemirror") || id.includes("@lezer")) return "codemirror";
          if (id.includes("mermaid")) return "mermaid";
          if (id.includes("@xterm")) return "terminal";
          if (id.includes("react") || id.includes("scheduler")) return "react";
          return undefined;
        }
      }
    }
  }
});
