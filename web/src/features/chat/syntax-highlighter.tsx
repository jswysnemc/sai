import { memo, useMemo, useSyncExternalStore } from "react";
import { api } from "../../api/client";
import { lineNumberDigitsStyle } from "./line-number-gutter";

const LANGUAGE_ALIASES: Record<string, string> = {
  cjs: "javascript",
  html: "xml",
  js: "javascript",
  jsx: "javascript",
  md: "markdown",
  diff: "diff",
  patch: "diff",
  py: "python",
  rs: "rust",
  shell: "bash",
  sh: "bash",
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
export const SyntaxHighlighter = memo(function SyntaxHighlighter({
  language,
  source,
  showLineNumbers = false
}: {
  language?: string;
  source: string;
  showLineNumbers?: boolean;
}) {
  const revision = useSyntectHighlightRevision();
  const result = useMemo(() => highlightSource(source, language), [source, language, revision]);
  const lines = useMemo(() => showLineNumbers ? splitHighlightedLines(result.value) : [], [result, showLineNumbers]);
  const className = `hljs${result.language ? ` language-${result.language}` : ""}`;
  if (!showLineNumbers) {
    return <code className={className} dangerouslySetInnerHTML={{ __html: result.value }} />;
  }
  return (
    <code className={`${className} syntax-lines`} style={lineNumberDigitsStyle(lines.length)}>
      {lines.map((line, index) => (
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
});

/**
 * 为代码块与差异视图生成统一的安全着色标记。
 * @param source 原始代码文本
 * @param language 文件语言或扩展名
 * @returns 经过转义的着色 HTML 及已确认的语言名称
 */
export function highlightSource(source: string, language?: string): { value: string; language: string } {
  const normalized = normalizeLanguage(language) || detectLanguage(source);
  if (source.length > 200_000) return { value: escapeHtml(source), language: normalized };
  const key = `${normalized}\u0000${source}`;
  const cached = htmlCache.get(key);
  if (cached !== undefined) return { value: cached, language: normalized };
  requestSyntectHighlight(source, normalized, key);
  return { value: escapeHtml(source), language: normalized };
}

/**
 * 把 syntect 返回的 HTML 写入缓存，供测试和已经着色过的文本复用。
 *
 * @param source 原始代码
 * @param language 语言标识
 * @param html syntect 生成的安全 HTML
 * @returns 无
 */
export function rememberSyntectHtml(source: string, language: string | undefined, html: string): void {
  const normalized = normalizeLanguage(language) || detectLanguage(source);
  htmlCache.set(`${normalized}\u0000${source}`, html);
  revision += 1;
  listeners.forEach((listener) => listener());
}

/**
 * 订阅着色缓存更新，结果返回后刷新正在显示的代码。
 *
 * @returns 递增的缓存版本
 */
export function useSyntectHighlightRevision(): number {
  return useSyncExternalStore(subscribeHighlight, () => revision, () => 0);
}

const htmlCache = new Map<string, string>();
const pending = new Set<string>();
const listeners = new Set<() => void>();
let revision = 0;

function subscribeHighlight(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

/**
 * 向后端请求 syntect 着色。测试环境不发请求。
 *
 * @param source 源码
 * @param language 规范化语言
 * @param key 缓存键
 * @returns 无
 */
function requestSyntectHighlight(source: string, language: string, key: string): void {
  if (pending.has(key) || import.meta.env.VITEST) return;
  pending.add(key);
  void api.highlight.code(source, language).then((result) => {
    htmlCache.set(key, result.html);
  }).catch(() => {
    htmlCache.set(key, escapeHtml(source));
  }).finally(() => {
    pending.delete(key);
    revision += 1;
    listeners.forEach((listener) => listener());
  });
}

/**
 * 在着色结果返回前转义源码，避免把原文当成 HTML。
 *
 * @param source 源码
 * @returns 转义后的文本
 */
function escapeHtml(source: string): string {
  return source.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

/**
 * 从文件路径推断语法着色语言，并统一处理 Windows 分隔符和补丁扩展名。
 *
 * @param path 文件路径
 * @returns Highlight.js 语言标识；无法推断时返回 undefined
 */
export function languageFromPath(path: string): string | undefined {
  const name = path.split(/[\\/]/u).filter(Boolean).at(-1) ?? "";
  if (!name.includes(".")) return undefined;
  const extension = name.split(".").at(-1)?.toLowerCase();
  return extension === "patch" || extension === "diff" ? "diff" : extension;
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
