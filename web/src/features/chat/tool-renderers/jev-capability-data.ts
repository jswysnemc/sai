import { text, type Locale } from "../../i18n/locale";
import { contextsOf, parseJevSelectedContext, type JevInjectedContext } from "./jev-context-data";
import { parseJsonRecord, type JsonRecord } from "./tool-data";

/** Jev 本次新暴露的一项工具或 Skill。 */
export type JevExposedResource = {
  kind: "tool" | "skill";
  name: string;
  /** 工具说明首句，或 skill 状态（loaded / already_loaded）。 */
  detail: string;
};

/** Jev 本轮暴露的资源与按需注入的上下文。 */
export type JevCapabilityExposure = {
  tools: JevExposedResource[];
  skills: JevExposedResource[];
  /** 命中的提示词片段与记忆；`request_capability` 结果中为空 */
  contexts: JevInjectedContext[];
};

/** 没有任何结果的暴露名单。 */
export const EMPTY_JEV_EXPOSURE: JevCapabilityExposure = { tools: [], skills: [], contexts: [] };

/**
 * 判断暴露名单是否为空。
 *
 * @param exposure 暴露名单
 * @returns 四类都为空时为 true
 */
export function isEmptyJevExposure(exposure: JevCapabilityExposure): boolean {
  return exposure.tools.length === 0 && exposure.skills.length === 0 && exposure.contexts.length === 0;
}

/**
 * 从用户消息注入前缀里取出本轮 Jev 预选结果。
 *
 * 工具与 Skill 在 `<jev-exposed-capabilities>`，提示词片段与记忆在
 * `<jev-selected-context>`，两块各自可选。任一标签存在就表示 Jev 被调用过；
 * JSON 解析失败时仍返回空名单，让界面能标出这次调用。
 *
 * @param content 供应商用户消息的注入前缀
 * @returns 暴露名单；两块都不存在时返回 null
 */
export function parseJevExposureBlock(content: string): JevCapabilityExposure | null {
  const match = /<jev-exposed-capabilities>([\s\S]*?)<\/jev-exposed-capabilities>/u.exec(content);
  const contexts = parseJevSelectedContext(content);
  if (!match && !contexts) return null;
  const body = match?.[1] ?? "";
  const jsonStart = body.indexOf("{");
  const jsonEnd = body.lastIndexOf("}");
  // 标签本身已说明来源，注入块里的 JSON 不要求 router 字段
  const resources = jsonStart >= 0 && jsonEnd > jsonStart
    ? parseExposureRecord(body.slice(jsonStart, jsonEnd + 1), false) ?? EMPTY_JEV_EXPOSURE
    : EMPTY_JEV_EXPOSURE;
  return { ...resources, contexts: contexts ?? [] };
}

/**
 * 从 `request_capability` 结果或预选 detail 中提取暴露名单。
 *
 * 只保留名称与一句说明。工具 Schema 和 Skill 全文留给模型，不进入界面；
 * 提示词片段与记忆只保留截断后的预览。
 *
 * @param output 工具输出或预选 detail
 * @returns 暴露名单；不是 Jev 成功结果时返回空
 */
export function parseJevCapability(output: string): JevCapabilityExposure | null {
  return parseExposureRecord(output, true);
}

/**
 * 解析暴露结果 JSON。
 *
 * @param output JSON 文本
 * @param requireRouter 是否要求 router 为 jev；工具结果需要据此与其它 JSON 区分
 * @returns 暴露名单；不是成功结果时返回空
 */
function parseExposureRecord(output: string, requireRouter: boolean): JevCapabilityExposure | null {
  const record = parseJsonRecord(output);
  if (!record || record.ok !== true) return null;
  if (requireRouter && record.router !== "jev") return null;
  return {
    tools: resourcesOf(record.tools, "tool"),
    skills: resourcesOf(record.skills, "skill"),
    contexts: contextsOf(record.contexts)
  };
}

/**
 * 折叠行右侧的暴露计数：工具、Skill、片段数量与记忆标记。
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
  const prompts = exposure.contexts.filter((item) => item.kind === "prompt").length;
  if (prompts > 0) {
    parts.push(text(locale, prompts === 1 ? "1 prompt" : `${prompts} prompts`, `${prompts} 个片段`));
  }
  if (exposure.contexts.some((item) => item.kind === "memory")) {
    parts.push(text(locale, "memory", "记忆"));
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
  if (typeof item.description === "string" && item.description.trim()) {
    return firstSentence(item.description);
  }
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
  if (typeof item.description === "string" && item.description.trim()) {
    return firstSentence(item.description);
  }
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
