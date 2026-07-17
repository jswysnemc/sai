export type FileMentionSegment =
  | { type: "text"; value: string }
  | { type: "mention"; path: string; value: string };

const MENTION_PATTERN = /(^|\s)@(?:"((?:\\.|[^"\\])+)"|([^\s]+))/gu;

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
 * 将文件路径格式化为后端可直接理解的 @ 引用文本。
 *
 * @param path 工作区相对文件路径
 * @returns 可插入输入内容的引用文本
 */
export function formatFileMention(path: string): string {
  if (!/\s/u.test(path)) return `@${path}`;
  return `@"${path.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

/**
 * 将输入文本拆分为普通文本和文件引用 token。
 *
 * @param value 输入框保存的后端文本
 * @returns 保持原始顺序的文本片段
 */
export function parseFileMentions(value: string): FileMentionSegment[] {
  const segments: FileMentionSegment[] = [];
  let cursor = 0;
  for (const match of value.matchAll(MENTION_PATTERN)) {
    const boundary = match[1] ?? "";
    const mentionStart = (match.index ?? 0) + boundary.length;
    if (mentionStart > cursor) segments.push({ type: "text", value: value.slice(cursor, mentionStart) });
    const raw = match[0].slice(boundary.length);
    const path = match[2] ? unescapeQuotedPath(match[2]) : match[3] ?? "";
    segments.push({ type: "mention", path, value: raw });
    cursor = mentionStart + raw.length;
  }
  if (cursor < value.length) segments.push({ type: "text", value: value.slice(cursor) });
  return segments;
}

/**
 * 还原带引号文件引用中的转义字符。
 *
 * @param path 带转义的路径正文
 * @returns 原始文件路径
 */
function unescapeQuotedPath(path: string): string {
  return path.replace(/\\([\\"])/gu, "$1");
}
