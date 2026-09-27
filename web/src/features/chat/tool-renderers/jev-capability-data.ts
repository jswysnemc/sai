import { text, type Locale } from "../../i18n/locale";
import { parseJsonRecord, type JsonRecord } from "./tool-data";

/** Jev 本次新暴露的一项工具或 Skill。 */
export type JevExposedResource = {
  kind: "tool" | "skill";
  name: string;
  /** 工具说明首句，或 skill 状态（loaded / already_loaded）。 */
  detail: string;
};

/** `request_capability` 成功结果里实际暴露的资源。 */
export type JevCapabilityExposure = {
  tools: JevExposedResource[];
  skills: JevExposedResource[];
};

/**
 * 从 `request_capability` 的结果 JSON 中提取暴露名单。
 *
 * 只保留名称与一句说明。工具 Schema 和 Skill 全文留给模型，不进入界面。
 *
 * @param output 工具输出
 * @returns 暴露名单；不是 Jev 成功结果时返回空
 */
/**
 * 从用户消息注入前缀里取出本轮 Jev 预选结果。
 *
 * 标签存在就表示 Jev 被调用过。JSON 解析失败时仍返回空名单，
 * 让界面能标出这次调用，而不是把它藏进原文。
 *
 * @param content 供应商用户消息的注入前缀
 * @returns 暴露名单；没有预选标签时返回空
 */
export function parseJevExposureBlock(content: string): JevCapabilityExposure | null {
  const match = /<jev-exposed-capabilities>([\s\S]*?)<\/jev-exposed-capabilities>/u.exec(content);
  if (!match) return null;
  const body = match[1] ?? "";
  const jsonStart = body.indexOf("{");
  const jsonEnd = body.lastIndexOf("}");
  if (jsonStart < 0 || jsonEnd < jsonStart) return { tools: [], skills: [] };
  return parseJevCapability(body.slice(jsonStart, jsonEnd + 1)) ?? { tools: [], skills: [] };
}

export function parseJevCapability(output: string): JevCapabilityExposure | null {
  const record = parseJsonRecord(output);
  if (!record || record.ok !== true || record.router !== "jev") return null;
  return {
    tools: resourcesOf(record.tools, "tool"),
    skills: resourcesOf(record.skills, "skill")
  };
}

/**
 * 折叠行右侧的暴露计数。
 *
 * @param exposure 已解析的暴露名单
 * @param locale 界面语言
 * @returns 计数或「未匹配」
 */
export function jevCapabilityStatusLabel(exposure: JevCapabilityExposure, locale: Locale): string {
  const parts: string[] = [];
  if (exposure.tools.length > 0) {
    const count = exposure.tools.length;
    parts.push(text(locale, count === 1 ? "1 tool" : `${count} tools`, `${count} 个工具`));
  }
  if (exposure.skills.length > 0) {
    const count = exposure.skills.length;
    parts.push(text(locale, count === 1 ? "1 skill" : `${count} skills`, `${count} 个 Skill`));
  }
  if (parts.length === 0) return text(locale, "no match", "未匹配");
  return parts.join(" · ");
}

/**
 * 把结果数组收成可展示的资源。
 *
 * @param value tools 或 skills 字段
 * @param kind 资源种类
 * @returns 去掉空名称后的名单
 */
function resourcesOf(value: unknown, kind: JevExposedResource["kind"]): JevExposedResource[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item) => {
    if (!isRecord(item)) return [];
    const name = typeof item.name === "string" ? item.name.trim() : "";
    if (!name) return [];
    const detail = kind === "tool" ? toolDetail(item) : skillDetail(item);
    return [{ kind, name, detail }];
  });
}

/**
 * 取工具定义里的说明首句。
 *
 * @param item 工具结果条目
 * @returns 首句说明；没有定义时为空
 */
function toolDetail(item: JsonRecord): string {
  const definition = item.definition;
  if (!isRecord(definition)) return "";
  const functionDef = definition.function;
  if (!isRecord(functionDef) || typeof functionDef.description !== "string") return "";
  return firstSentence(functionDef.description);
}

/**
 * 取 skill 的加载状态，不带回全文。
 *
 * @param item skill 结果条目
 * @returns loaded、already_loaded 或空
 */
function skillDetail(item: JsonRecord): string {
  return typeof item.status === "string" ? item.status.trim() : "";
}

/**
 * 把说明压成一句，避免卡片里铺开整段工具描述。
 *
 * @param value 原始说明
 * @returns 首句
 */
function firstSentence(value: string): string {
  const single = value.replace(/\s+/g, " ").trim();
  const sentence = single.split(/(?<=[。.!？?])\s/u)[0] ?? single;
  if (sentence.length <= 140) return sentence;
  return `${sentence.slice(0, 139)}…`;
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
