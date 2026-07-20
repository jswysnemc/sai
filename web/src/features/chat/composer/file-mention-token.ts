export type FileMentionSegment =
  | { type: "text"; value: string }
  | { type: "mention"; path: string; value: string };

const FILE_REFERENCE_PATTERN = /<file-reference path="((?:\\.|[^"\\])*)"><\/file-reference>/gu;

export type FileMentionTriggerRange = {
  start: number;
  end: number;
};

/**
 * 查找光标前刚输入的文件引用触发符。
 *
 * @param value 输入区当前纯文本
 * @param caret 当前光标偏移
 * @param insertedText 本次输入事件插入的文本
 * @returns 触发符范围，光标前不是 @ 时返回 null
 */
export function findFileMentionTrigger(value: string, caret: number, insertedText: string | null): FileMentionTriggerRange | null {
  if (insertedText !== "@") return null;
  if (!Number.isInteger(caret) || caret <= 0 || caret > value.length) return null;
  const start = caret - 1;
  return value[start] === "@" ? { start, end: caret } : null;
}

/**
 * 将文件路径格式化为成功引用协议，仅选择器确认后插入。
 *
 * @param path 工作区相对文件路径
 * @returns 可插入输入内容的成功引用文本
 */
export function formatFileMention(path: string): string {
  return `<file-reference path="${escapeXmlAttr(path.trim())}"></file-reference>`;
}

/**
 * 将输入文本拆分为普通文本和成功选择的文件引用。
 *
 * 手写 `@path` 保持普通文本，只有选择器生成的 file-reference 会特殊渲染。
 *
 * @param value 输入框保存的后端文本
 * @returns 保持原始顺序的文本片段
 */
export function parseFileMentions(value: string): FileMentionSegment[] {
  const segments: FileMentionSegment[] = [];
  let cursor = 0;
  for (const match of value.matchAll(FILE_REFERENCE_PATTERN)) {
    const start = match.index ?? 0;
    if (start > cursor) segments.push({ type: "text", value: value.slice(cursor, start) });
    const raw = match[0];
    const path = unescapeXmlAttr(match[1] ?? "");
    segments.push({ type: "mention", path, value: raw });
    cursor = start + raw.length;
  }
  if (cursor < value.length) segments.push({ type: "text", value: value.slice(cursor) });
  return segments;
}

/** 将路径写入 XML 属性。 */
function escapeXmlAttr(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll('"', "&quot;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

/** 还原 XML 属性中的路径。 */
function unescapeXmlAttr(value: string): string {
  return value
    .replaceAll("&quot;", '"')
    .replaceAll("&lt;", "<")
    .replaceAll("&gt;", ">")
    .replaceAll("&amp;", "&");
}
