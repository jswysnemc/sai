/**
 * 为单文件编辑保留语法检查，关闭依赖项目类型上下文的诊断。
 * @param monaco 已加载的 Monaco API
 * @returns 无返回值
 */
export function configureStandaloneTypeScript(monaco: typeof import("monaco-editor")): void {
  // 1. 当前只加载打开文件，缺少 tsconfig 和依赖类型时无法准确判断语义错误
  const { javascriptDefaults, typescriptDefaults } = monaco.languages.typescript;
  for (const defaults of [javascriptDefaults, typescriptDefaults]) {
    defaults.setDiagnosticsOptions({
      ...defaults.getDiagnosticsOptions(),
      noSemanticValidation: true,
      noSyntaxValidation: false,
      noSuggestionDiagnostics: true
    });
  }
}
