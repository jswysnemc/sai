import CssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import HtmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import JsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import TypeScriptWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

declare global {
  // Monaco 在缺少 AMD loader 时用它拼模块 URI，避免 toUrl 读 undefined
  // eslint-disable-next-line no-var
  var _VSCODE_FILE_ROOT: string | undefined;
}

type MonacoApi = typeof import("monaco-editor");
let diffLanguageConfigured = false;

/** 配置 Monaco 使用 Vite 打包的语言 Worker，并补齐浏览器 URI 根路径。 */
export function configureMonacoEnvironment(): void {
  // 1. Vite/ESM 没有 require.toUrl，需提供根路径给 FileAccess.toUri
  if (typeof globalThis._VSCODE_FILE_ROOT !== "string" || !globalThis._VSCODE_FILE_ROOT) {
    globalThis._VSCODE_FILE_ROOT = new URL("/", window.location.href).href;
  }

  if (window.MonacoEnvironment?.getWorker) return;

  window.MonacoEnvironment = {
    getWorker(_moduleId, label) {
      // 2. 按语言选择专用 Worker，其余语言使用基础编辑器 Worker
      if (label === "json") return new JsonWorker();
      if (label === "css" || label === "scss" || label === "less") return new CssWorker();
      if (label === "html" || label === "handlebars" || label === "razor") return new HtmlWorker();
      if (label === "typescript" || label === "javascript") return new TypeScriptWorker();
      return new EditorWorker();
    }
  };
}

/**
 * 注册工作区补丁文件使用的轻量 diff 语言和主题。
 *
 * @param monaco Monaco API 实例
 * @returns 无返回值
 */
export function configureDiffLanguage(monaco: MonacoApi): void {
  if (diffLanguageConfigured) return;
  diffLanguageConfigured = true;
  monaco.languages.register({ id: "diff" });
  monaco.languages.setMonarchTokensProvider("diff", {
    tokenizer: {
      root: [
        [/^@@.*$/u, "diff-hunk"],
        [/^diff --git.*$/u, "diff-command"],
        [/^(?:\+\+\+|---|index ).*$/u, "diff-meta"],
        [/^\+.*$/u, "diff-added"],
        [/^-.*$/u, "diff-removed"]
      ]
    }
  });
  monaco.editor.defineTheme("sai-light", {
    base: "vs",
    inherit: true,
    rules: [
      { token: "diff-added", foreground: "1A7F37", background: "E6FFEC" },
      { token: "diff-removed", foreground: "CF222E", background: "FFEBE9" },
      { token: "diff-hunk", foreground: "8250DF", fontStyle: "bold" },
      { token: "diff-command", foreground: "0550AE", fontStyle: "bold" },
      { token: "diff-meta", foreground: "57606A" }
    ],
    colors: {}
  });
  monaco.editor.defineTheme("sai-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "diff-added", foreground: "56D364", background: "12261B" },
      { token: "diff-removed", foreground: "FF7B72", background: "3A1E20" },
      { token: "diff-hunk", foreground: "D2A8FF", fontStyle: "bold" },
      { token: "diff-command", foreground: "79C0FF", fontStyle: "bold" },
      { token: "diff-meta", foreground: "8B949E" }
    ],
    colors: {}
  });
}
