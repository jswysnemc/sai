import type { JsonRecord } from "./tool-data";

/** Jev 本轮按需注入的一段上下文：提示词片段或记忆。 */
export type JevInjectedContext = {
  kind: "prompt" | "memory";
  id: string;
  /** 来源：system、instructions、extra 或 memory */
  source: string;
  /** 片段说明 */
  description: string;
  /** 注入正文预览；记忆为索引条目 */
  preview: string;
};

/** 界面预览正文的最大字符数，与后端 detail 截断保持同一量级。 */
const PREVIEW_CHARS = 1200;

/**
 * 读取预选 detail 中的 contexts 数组。
 *
 * @param value contexts 字段
 * @returns 可展示的片段与记忆
 */
export function contextsOf(value: unknown): JevInjectedContext[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (!isRecord(item)) return [];
    const kind = item.kind === "memory" ? "memory" : item.kind === "prompt" ? "prompt" : null;
    if (!kind) return [];
    return [{
      kind,
      id: stringOf(item.id),
      source: stringOf(item.source),
      description: stringOf(item.description),
      preview: stringOf(item.preview)
    }];
  });
}

/**
 * 从历史用户消息的注入前缀里取出 `<jev-selected-context>` 中的片段。
 *
 * 新标签带 kind、source、description 属性；旧会话只有 id，按 id 推断种类。
 *
 * @param content 注入前缀
 * @returns 片段列表；没有该块时为 null
 */
export function parseJevSelectedContext(content: string): JevInjectedContext[] | null {
  const block = /<jev-selected-context>([\s\S]*?)<\/jev-selected-context>/u.exec(content);
  if (!block) return null;
  const contexts: JevInjectedContext[] = [];
  const memoryIndex = memoryEntries(content);
  const pattern = /<jev-context\s+([^>]*)>([\s\S]*?)<\/jev-context>/gu;
  for (const match of (block[1] ?? "").matchAll(pattern)) {
    const attributes = parseAttributes(match[1] ?? "");
    const id = attributes.id ?? "";
    const kind = attributes.kind === "memory" || (!attributes.kind && id === "memory_context") ? "memory" : "prompt";
    contexts.push({
      kind,
      id,
      source: attributes.source ?? "",
      description: attributes.description ?? "",
      // 记忆块正文是使用契约；有价值的是同一前缀里注入的索引条目
      preview: kind === "memory" ? memoryIndex : truncate((match[2] ?? "").trim())
    });
  }
  return contexts;
}

/**
 * 从注入前缀的 `<memory>` 块中取出分组标题与条目行。
 *
 * @param content 注入前缀
 * @returns 记忆条目；没有索引时为空
 */
function memoryEntries(content: string): string {
  const block = /<memory>([\s\S]*?)<\/memory>/u.exec(content);
  if (!block) return "";
  return (block[1] ?? "")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.startsWith("-") || line.endsWith("：") || line.endsWith(":"))
    .join("\n");
}

/**
 * 片段来源的界面文字。
 *
 * @param source 来源标识
 * @param t 双语文本选择方法
 * @returns 来源说明
 */
export function contextSourceLabel(source: string, t: (en: string, zh: string) => string): string {
  if (source === "system") return t("system prompt", "系统提示");
  if (source === "instructions") return t("instruction files", "指令文件");
  if (source === "extra") return t("extra prompt", "附加提示");
  if (source === "memory") return t("memory", "记忆");
  return source;
}

/**
 * 解析标签属性，并还原后端做过的实体转义。
 *
 * @param text 标签内的属性文本
 * @returns 属性名到值的映射
 */
function parseAttributes(text: string): Record<string, string> {
  const result: Record<string, string> = {};
  for (const match of text.matchAll(/([a-z_]+)="([^"]*)"/gu)) {
    result[match[1] ?? ""] = unescape(match[2] ?? "");
  }
  return result;
}

/**
 * 还原属性值里的 HTML 实体。
 *
 * @param value 转义后的值
 * @returns 原文
 */
function unescape(value: string): string {
  return value
    .replaceAll("&quot;", "\"")
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&amp;", "&");
}

/**
 * 截断过长的预览。
 *
 * @param value 原文
 * @returns 截断后的文本
 */
function truncate(value: string): string {
  return value.length <= PREVIEW_CHARS ? value : `${value.slice(0, PREVIEW_CHARS)}…`;
}

/**
 * 读取字符串字段。
 *
 * @param value 字段值
 * @returns 去掉首尾空白的字符串
 */
function stringOf(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

/**
 * 判断未知值是否为普通对象。
 *
 * @param value 待判断值
 * @returns 是否可按字段读取
 */
function isRecord(value: unknown): value is JsonRecord {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
