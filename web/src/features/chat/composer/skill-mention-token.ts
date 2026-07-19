export type SkillMentionSegment =
  | { type: "text"; value: string }
  | { type: "skill"; name: string; value: string };

export type ExpandedSkillReferenceSegment =
  | { type: "text"; value: string }
  | { type: "skill_reference"; name: string; content: string; value: string };

export type SkillMentionTriggerRange = {
  start: number;
  end: number;
  query: string;
};

const SKILL_PATTERN = /(^|\s)\/([A-Za-z0-9][A-Za-z0-9._-]*)/gu;
const EXPANDED_SKILL_PATTERN = /<skill-reference name="([A-Za-z0-9][A-Za-z0-9._-]*)">([\s\S]*?)<\/skill-reference>/gu;

/**
 * 查找光标前的 skill 斜杠触发范围。
 *
 * 触发条件：光标位于 `/name` 片段末尾，且 `/` 前是行首或空白。
 *
 * @param value 输入区当前纯文本
 * @param caret 当前光标偏移
 * @returns 触发范围与过滤词；未触发时返回 null
 */
export function findSkillMentionTrigger(value: string, caret: number): SkillMentionTriggerRange | null {
  if (!Number.isInteger(caret) || caret <= 0 || caret > value.length) return null;
  const before = value.slice(0, caret);
  const match = before.match(/(^|[\s\n])\/([A-Za-z0-9._-]*)$/u);
  if (!match) return null;
  const token = match[0];
  const leading = match[1] ?? "";
  const start = caret - token.length + leading.length;
  const query = match[2] ?? "";
  return { start, end: caret, query };
}

/**
 * 将 skill 名称格式化为输入区 token 文本。
 *
 * @param name skill 名称
 * @returns 可插入输入内容的 skill 引用
 */
export function formatSkillMention(name: string): string {
  return `/${name.trim()}`;
}

/**
 * 将输入文本拆分为普通文本与 skill 引用 token。
 *
 * @param value 输入框保存的后端文本
 * @returns 保持原始顺序的文本片段
 */
export function parseSkillMentions(value: string): SkillMentionSegment[] {
  const segments: SkillMentionSegment[] = [];
  let cursor = 0;
  for (const match of value.matchAll(SKILL_PATTERN)) {
    const boundary = match[1] ?? "";
    const mentionStart = (match.index ?? 0) + boundary.length;
    if (mentionStart > cursor) segments.push({ type: "text", value: value.slice(cursor, mentionStart) });
    const name = match[2] ?? "";
    const raw = `/${name}`;
    segments.push({ type: "skill", name, value: raw });
    cursor = mentionStart + raw.length;
  }
  if (cursor < value.length) segments.push({ type: "text", value: value.slice(cursor) });
  return segments;
}

/**
 * 提取输入中引用的 skill 名称，按出现顺序去重。
 *
 * @param value 输入文本
 * @returns skill 名称列表
 */
export function collectSkillMentionNames(value: string): string[] {
  const names: string[] = [];
  const seen = new Set<string>();
  for (const segment of parseSkillMentions(value)) {
    if (segment.type !== "skill") continue;
    if (seen.has(segment.name)) continue;
    seen.add(segment.name);
    names.push(segment.name);
  }
  return names;
}

/**
 * 将展开后的 Skill 文档编码为可持久化引用。
 *
 * @param name Skill 名称
 * @param content 完整 Skill 文档
 * @returns 同时供模型读取和气泡还原的引用文本
 */
export function formatExpandedSkillReference(name: string, content: string): string {
  const safeContent = content.trim().replaceAll("</skill-reference>", "<\\/skill-reference>");
  return `<skill-reference name="${name.trim()}">\n${safeContent}\n</skill-reference>`;
}

/**
 * 解析展开后的 Skill 引用，并保留引用之间的普通文本。
 *
 * @param value 已发送或持久化的用户输入
 * @returns 普通文本和完整 Skill 引用片段
 */
export function parseExpandedSkillReferences(value: string): ExpandedSkillReferenceSegment[] {
  const segments: ExpandedSkillReferenceSegment[] = [];
  let cursor = 0;
  for (const match of value.matchAll(EXPANDED_SKILL_PATTERN)) {
    const start = match.index ?? 0;
    if (start > cursor) segments.push({ type: "text", value: value.slice(cursor, start) });
    const raw = match[0];
    const content = (match[2] ?? "")
      .replace(/^\n/u, "")
      .replace(/\n$/u, "")
      .replaceAll("<\\/skill-reference>", "</skill-reference>");
    segments.push({
      type: "skill_reference",
      name: match[1] ?? "",
      content,
      value: raw
    });
    cursor = start + raw.length;
  }
  if (cursor < value.length) segments.push({ type: "text", value: value.slice(cursor) });
  return segments;
}

/**
 * 将输入中的 `/skill` 引用替换为完整 skill 文档，再发给模型。
 *
 * 输入区仍保留短 token，仅发送路径展开完整内容。
 *
 * @param value 用户输入
 * @param documents skill 名称到完整文档的映射
 * @returns 展开后的模型输入
 */
export function expandSkillMentions(value: string, documents: Record<string, string>): string {
  return parseSkillMentions(value)
    .map((segment) => {
      if (segment.type === "text") return segment.value;
      const document = documents[segment.name];
      return document?.trim() ? formatExpandedSkillReference(segment.name, document) : segment.value;
    })
    .join("");
}
