/** 文件扩展名到 Monaco 语言标识的映射。 */
const LANGUAGE_BY_EXTENSION: Record<string, string> = {
  rs: "rust",
  ts: "typescript",
  tsx: "typescript",
  js: "javascript",
  jsx: "javascript",
  json: "json",
  md: "markdown",
  css: "css",
  html: "html",
  py: "python",
  go: "go",
  sh: "shell",
  toml: "ini",
  yaml: "yaml",
  yml: "yaml",
};

/**
 * 按文件路径推断编辑器语言。
 *
 * @param path 文件路径
 * @returns Monaco 语言标识，未识别时为 plaintext
 */
export function languageForPath(path: string): string {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  return LANGUAGE_BY_EXTENSION[extension] ?? "plaintext";
}
