/** 单行摘要的最大字符数；超出部分由 CSS 省略号收尾。 */
const SUMMARY_LIMIT = 220;

/**
 * 把多行内容压成单行摘要。
 *
 * @param content 原始内容
 * @returns 去除换行与多余空白的单行文本
 */
export function summarizeContent(content: string): string {
  const text = content.replace(/\s+/g, " ").trim();
  return text.length > SUMMARY_LIMIT ? `${text.slice(0, SUMMARY_LIMIT)}…` : text;
}

/**
 * 把工具入参压成可读摘要。
 *
 * JSON 入参展开成 `键=值` 序列而不是原样打印：原样的花括号与引号
 * 在窄列里几乎占满宽度，真正有区分度的是参数值。
 *
 * @param args 工具入参文本
 * @returns 单行摘要
 */
export function summarizeToolArguments(args: string): string {
  const trimmed = args.trim();
  if (!trimmed) return "";
  try {
    const parsed: unknown = JSON.parse(trimmed);
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) {
      return summarizeContent(String(parsed));
    }
    const parts = Object.entries(parsed as Record<string, unknown>)
      .map(([key, value]) => `${key}=${summarizeValue(value)}`);
    // 用间隔号而非空格分隔：空格会被 summarizeContent 的空白压缩吃掉，
    // 参数之间就和参数值内部的空格混在一起了
    return summarizeContent(parts.join(" · "));
  } catch {
    return summarizeContent(trimmed);
  }
}

/**
 * 把入参的单个值压成短文本。
 *
 * @param value 参数值
 * @returns 短文本表示
 */
function summarizeValue(value: unknown): string {
  if (typeof value === "string") return value.replace(/\s+/g, " ").trim();
  if (Array.isArray(value)) return `[${value.length}]`;
  if (value !== null && typeof value === "object") return "{…}";
  return String(value);
}

/**
 * 格式化耗时。
 *
 * @param milliseconds 耗时毫秒；未知时传 null
 * @returns 带单位的耗时文本；未知时返回短横线
 */
export function formatDuration(milliseconds: number | null | undefined): string {
  if (milliseconds == null || !Number.isFinite(milliseconds)) return "-";
  if (milliseconds < 1000) return `${Math.round(milliseconds)}ms`;
  const seconds = milliseconds / 1000;
  if (seconds < 60) return `${seconds.toFixed(seconds < 10 ? 2 : 1)}s`;
  const minutes = Math.floor(seconds / 60);
  return `${minutes}m${String(Math.round(seconds - minutes * 60)).padStart(2, "0")}s`;
}

/**
 * 格式化时刻。
 *
 * @param timestamp 毫秒时间戳；未知时传 null
 * @param locale 界面语言
 * @returns 时分秒文本；未知时返回短横线
 */
export function formatClock(timestamp: number | null | undefined, locale: string): string {
  if (timestamp == null) return "-";
  return new Date(timestamp).toLocaleTimeString(locale, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit"
  });
}

/**
 * 格式化 token 数量。
 *
 * @param value token 数
 * @returns 千分位或 k 缩写文本
 */
export function formatTokens(value: number): string {
  if (value < 1000) return String(value);
  if (value < 10_000) return `${(value / 1000).toFixed(1)}k`;
  return `${Math.round(value / 1000)}k`;
}

/**
 * 尝试把文本格式化为缩进 JSON。
 *
 * @param text 原始文本
 * @returns 缩进后的 JSON；解析失败时原样返回
 */
export function prettyJson(text: string): string {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}
