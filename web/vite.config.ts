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
    manifest: true,
    rollupOptions: {
      output: {
        onlyExplicitManualChunks: true,
        /**
         * 【前端性能】【依赖分包】只归并目标库本身，保持共享依赖与包装层的加载边界。
         * @param id 构建模块的完整路径
         * @returns 手动分包名称，其余模块由构建器拆分
         */
        manualChunks(id) {
          const path = id.replace(/\\/g, "/");
          if (path.includes("/node_modules/monaco-editor/") && !path.includes("?worker")) return "monaco";
          // 【前端性能】【依赖顺序】CodeMirror、Lezer 与 Mermaid 保留原生拆分，避免语言解析器循环初始化或提前载入布局引擎
          if (id.includes("@xterm")) return "terminal";
          if (/\/node_modules\/(?:react|react-dom|scheduler)\//.test(path)) return "react";
          return undefined;
        }
      }
    }
  }
});
