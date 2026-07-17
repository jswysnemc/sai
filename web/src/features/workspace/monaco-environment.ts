import CssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import EditorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import HtmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import JsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import TypeScriptWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

/** 配置 Monaco 使用 Vite 打包的语言 Worker。 */
export function configureMonacoEnvironment(): void {
  if (window.MonacoEnvironment) return;
  window.MonacoEnvironment = {
    getWorker(_moduleId, label) {
      // 1. 按语言选择专用 Worker，其余语言使用基础编辑器 Worker
      if (label === "json") return new JsonWorker();
      if (label === "css" || label === "scss" || label === "less") return new CssWorker();
      if (label === "html" || label === "handlebars" || label === "razor") return new HtmlWorker();
      if (label === "typescript" || label === "javascript") return new TypeScriptWorker();
      return new EditorWorker();
    }
  };
}
