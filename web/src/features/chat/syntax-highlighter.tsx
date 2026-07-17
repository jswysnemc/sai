import hljs from "highlight.js/lib/core";
import bash from "highlight.js/lib/languages/bash";
import css from "highlight.js/lib/languages/css";
import diff from "highlight.js/lib/languages/diff";
import go from "highlight.js/lib/languages/go";
import ini from "highlight.js/lib/languages/ini";
import java from "highlight.js/lib/languages/java";
import javascript from "highlight.js/lib/languages/javascript";
import json from "highlight.js/lib/languages/json";
import markdown from "highlight.js/lib/languages/markdown";
import python from "highlight.js/lib/languages/python";
import rust from "highlight.js/lib/languages/rust";
import sql from "highlight.js/lib/languages/sql";
import typescript from "highlight.js/lib/languages/typescript";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";

hljs.registerLanguage("bash", bash);
hljs.registerLanguage("css", css);
hljs.registerLanguage("diff", diff);
hljs.registerLanguage("go", go);
hljs.registerLanguage("ini", ini);
hljs.registerLanguage("java", java);
hljs.registerLanguage("javascript", javascript);
hljs.registerLanguage("json", json);
hljs.registerLanguage("markdown", markdown);
hljs.registerLanguage("python", python);
hljs.registerLanguage("rust", rust);
hljs.registerLanguage("sql", sql);
hljs.registerLanguage("typescript", typescript);
hljs.registerLanguage("xml", xml);
hljs.registerLanguage("yaml", yaml);

const LANGUAGE_ALIASES: Record<string, string> = {
  cjs: "javascript",
  html: "xml",
  js: "javascript",
  jsx: "javascript",
  md: "markdown",
  py: "python",
  rs: "rust",
  shell: "bash",
  sh: "bash",
  toml: "ini",
  ts: "typescript",
  tsx: "typescript",
  yml: "yaml"
};

/**
 * 使用受控语言集合生成代码着色标记。
 *
 * @param props 代码语言和源代码
 * @returns 带语法分类的代码元素
 */
export function SyntaxHighlighter({ language, source }: { language?: string; source: string }) {
  const normalized = normalizeLanguage(language);
  const result = normalized && hljs.getLanguage(normalized)
    ? hljs.highlight(source, { language: normalized, ignoreIllegals: true })
    : hljs.highlightAuto(source);
  return <code className={`hljs${normalized ? ` language-${normalized}` : ""}`} dangerouslySetInnerHTML={{ __html: result.value }} />;
}

/**
 * 将 Markdown 语言别名转换为高亮器注册名称。
 *
 * @param language Markdown 代码围栏语言
 * @returns 标准语言名称
 */
function normalizeLanguage(language?: string): string {
  const value = language?.trim().toLowerCase() ?? "";
  return LANGUAGE_ALIASES[value] ?? value;
}
