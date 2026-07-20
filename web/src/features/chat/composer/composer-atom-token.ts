import { parseFileMentions } from "./file-mention-token";
import { parseExpandedSkillReferences, parseSelectedSkillMentions } from "./skill-mention-token";

export type ComposerAtomSegment =
  | { type: "text"; value: string }
  | { type: "file"; path: string; value: string }
  | { type: "skill"; name: string; content?: string; value: string }
  | { type: "goal"; value: string }
  | { type: "terminal"; source: string; content: string; value: string };

const TERMINAL_SELECTION_PATTERN = /<terminal-selection source="([^"]*)">([\s\S]*?)<\/terminal-selection>/gu;

/**
 * 将输入文本拆分为普通文本、文件、Skill、Goal 和终端选区原子。
 *
 * @param value 输入框保存的纯文本协议
 * @returns 保持原始顺序的输入片段
 */
export function parseComposerAtoms(value: string): ComposerAtomSegment[] {
  const segments: ComposerAtomSegment[] = [];
  for (const terminalSegment of parseTerminalSelections(value)) {
    if (terminalSegment.type === "terminal") {
      segments.push(terminalSegment);
      continue;
    }
    for (const skillReference of parseExpandedSkillReferences(terminalSegment.value)) {
      if (skillReference.type === "skill_reference") {
        segments.push({
          type: "skill",
          name: skillReference.name,
          content: skillReference.content,
          value: skillReference.value
        });
        continue;
      }
      for (const fileSegment of parseFileMentions(skillReference.value)) {
        if (fileSegment.type === "mention") {
          segments.push({ type: "file", path: fileSegment.path, value: fileSegment.value });
          continue;
        }
        for (const skillSegment of parseSelectedSkillMentions(fileSegment.value)) {
          if (skillSegment.type === "text") {
            // 仅当整段以 /goal 命令形式出现时保留 goal 特殊渲染
            for (const goalSegment of parseGoalCommandAtoms(skillSegment.value)) {
              if (goalSegment.value) segments.push(goalSegment);
            }
          } else if (skillSegment.name === "goal") {
            // 兼容旧草稿里误写成 skill-mention 的 /goal
            segments.push({ type: "goal", value: "/goal" });
          } else {
            segments.push({ type: "skill", name: skillSegment.name, value: skillSegment.value });
          }
        }
      }
    }
  }
  return segments;
}

/**
 * 将终端选区编码为可持久化到输入草稿的文本协议。
 *
 * @param source 终端标题
 * @param content 选中的终端文本
 * @returns 可由输入原子解析器还原的文本
 */
export function formatTerminalSelection(source: string, content: string): string {
  return `<terminal-selection source="${escapeXml(source)}">${escapeXml(content)}</terminal-selection>`;
}

/**
 * 解析终端选区原子，并保留其间普通文本。
 *
 * @param value 输入文本
 * @returns 终端选区与普通文本片段
 */
function parseTerminalSelections(value: string): ComposerAtomSegment[] {
  const segments: ComposerAtomSegment[] = [];
  let cursor = 0;
  for (const match of value.matchAll(TERMINAL_SELECTION_PATTERN)) {
    const start = match.index ?? 0;
    if (start > cursor) segments.push({ type: "text", value: value.slice(cursor, start) });
    segments.push({
      type: "terminal",
      source: unescapeXml(match[1] ?? ""),
      content: unescapeXml(match[2] ?? ""),
      value: match[0]
    });
    cursor = start + match[0].length;
  }
  if (cursor < value.length) segments.push({ type: "text", value: value.slice(cursor) });
  return segments;
}

/** 将文本转义为终端选区协议可安全保存的 XML 内容。 */
function escapeXml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

/** 将终端选区协议中的 XML 实体还原为原始文本。 */
function unescapeXml(value: string): string {
  return value
    .replaceAll("&quot;", '"')
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&amp;", "&");
}

/**
 * 仅把独立的 `/goal` 命令 token 标为 goal 原子，其余手写斜杠文本保持原样。
 *
 * @param value 普通文本片段
 * @returns 文本与可选 goal 原子
 */
function parseGoalCommandAtoms(value: string): Array<ComposerAtomSegment> {
  const pattern = /(^|\s)(\/goal)(?=\s|$)/gu;
  const segments: ComposerAtomSegment[] = [];
  let cursor = 0;
  for (const match of value.matchAll(pattern)) {
    const boundary = match[1] ?? "";
    const start = (match.index ?? 0) + boundary.length;
    if (start > cursor) segments.push({ type: "text", value: value.slice(cursor, start) });
    segments.push({ type: "goal", value: "/goal" });
    cursor = start + 5;
  }
  if (cursor < value.length) segments.push({ type: "text", value: value.slice(cursor) });
  return segments;
}
