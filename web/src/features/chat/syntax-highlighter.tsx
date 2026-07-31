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
export function SyntaxHighlighter({
  language,
  source,
  showLineNumbers = false
}: {
  language?: string;
  source: string;
  showLineNumbers?: boolean;
}) {
  const normalized = normalizeLanguage(language);
  const resolved = normalized && hljs.getLanguage(normalized) ? normalized : detectLanguage(source);
  const result = resolved
    ? hljs.highlight(source, { language: resolved, ignoreIllegals: true })
    : hljs.highlightAuto(source, AUTO_DETECT_LANGUAGES);
  const className = `hljs${resolved ? ` language-${resolved}` : ""}`;
  if (!showLineNumbers) {
    return <code className={className} dangerouslySetInnerHTML={{ __html: result.value }} />;
  }
  return (
    <code className={`${className} syntax-lines`}>
      {splitHighlightedLines(result.value).map((line, index) => (
        <span className="syntax-line" key={index}>
          <span className="syntax-line-number" aria-hidden="true">{index + 1}</span>
          <span
            className="syntax-line-content"
            dangerouslySetInnerHTML={{ __html: line || "&#8203;" }}
          />
        </span>
      ))}
    </code>
  );
}

/**
 * 按换行符拆分 Highlight.js 标记，并在每一行闭合后重新打开跨行标签。
 *
 * @param html Highlight.js 生成的安全高亮标记
 * @returns 每行可独立渲染的闭合标记
 */
export function splitHighlightedLines(html: string): string[] {
  const tokens = html.split(/(<span\b[^>]*>|<\/span>|\r?\n)/gu);
  const lines = [""];
  const openTags: string[] = [];

  tokens.forEach((token) => {
    if (!token) return;
    if (token === "\n" || token === "\r\n") {
      lines[lines.length - 1] += "</span>".repeat(openTags.length);
      lines.push(openTags.join(""));
      return;
    }
    if (token.startsWith("<span")) {
      openTags.push(token);
      lines[lines.length - 1] += token;
      return;
    }
    if (token === "</span>") {
      openTags.pop();
      lines[lines.length - 1] += token;
      return;
    }
    lines[lines.length - 1] += token;
  });

  return lines;
}

/**
 * 自动检测时允许参与的语言。
 *
 * 刻意排除 diff：它的特征是"行首是 + 或 -"，几乎任何缩进文本都能碰上，
 * 一旦误判，整段内容会被套上 hljs-addition 的绿色底纹（读文件结果曾因此长满色块）。
 * 无扩展名的路径本来就走自动检测，所以必须在候选集合这一层挡掉。
 */
const AUTO_DETECT_LANGUAGES = [
  "json",
  "bash",
  "yaml",
  "typescript",
  "javascript",
  "python",
  "rust",
  "go",
  "xml",
  "markdown"
];

/**
 * 在没有可靠语言标识时按内容特征判定语言。
 *
 * 只处理能确定的情况，判不出来时返回空串交给受限的自动检测。
 *
 * @param source 待着色的源码
 * @returns 语言标识，无法判定时返回空串
 */
function detectLanguage(source: string): string {
  const head = source.trimStart();
  if (!head) return "";
  // 1. 成对的花括号或方括号且能解析，按 JSON 处理
  if (head.startsWith("{") || head.startsWith("[")) {
    try {
      JSON.parse(source);
      return "json";
    } catch {
      return "";
    }
  }
  // 2. 统一 diff 的头部标记明确，可以直接确认
  if (head.startsWith("diff --git") || head.startsWith("--- ") || head.startsWith("@@ ")) return "diff";
  return "";
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
