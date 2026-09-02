/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  // 【Web 测试】【CI 稳定性】放宽 vitest 超时并限制并发 worker。
  // Windows runner 上 191 个测试文件并发导入（import 合计 37s）时，
  // 单个同步用例的计时会包含模块加载排队时间，偶发超过默认 5s；
  // 解析器类用例本身只耗时十几毫秒，瓶颈在调度而非代码。
  test: {
    testTimeout: 30_000,
    hookTimeout: 30_000,
    maxWorkers: "50%"
  },
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
