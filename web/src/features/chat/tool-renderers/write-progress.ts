/*
 * 写入进度估算
 *
 * 编辑类工具执行时参数以 JSON 文本流式抵达，其中换行以 `\n` 转义序列
 * 出现。统计转义换行数即可近似"已经写到第几行"，供折叠行的
 * 数字动效使用，不必等参数闭合成合法 JSON。
 *
 * 与 TUI `streamed_diff_counts` 对齐：str_replace 分 old/new 计删增，
 * write_file 的 content 全算新增，供 +N -M 徽章实时跳动。
 */

/** 流式增删近似统计 */
export type StreamedDiffCounts = {
  added: number;
  removed: number;
};

/**
 * 统计参数流中已出现的转义换行数。
 *
 * @param argumentsText 工具参数原始文本（可能是不完整的 JSON 前缀）
 * @returns 转义换行数量，可视作已写入的行数
 */
export function streamedLineCount(argumentsText: string): number {
  if (!argumentsText) return 0;
  let count = 0;
  let index = argumentsText.indexOf("\\n");
  while (index !== -1) {
    count += 1;
    index = argumentsText.indexOf("\\n", index + 2);
  }
  return count;
}

/**
 * 从（可能未闭合的）编辑工具参数流中近似统计增删行数。
 *
 * `str_replace`：old_string 计删、new_string 计增；
 * `write_file` / `edit_file`：content 全按新增。
 *
 * @param argumentsText 工具参数原始文本（可能是不完整的 JSON 前缀）
 * @returns 增删统计；尚无可统计字段时返回 null
 */
export function streamedDiffCounts(argumentsText: string): StreamedDiffCounts | null {
  if (!argumentsText) return null;
  const removed = lenientFieldLineCount(argumentsText, "old_string");
  const added = lenientFieldLineCount(argumentsText, "new_string");
  if (removed !== null || added !== null) {
    return { added: added ?? 0, removed: removed ?? 0 };
  }
  const content = lenientFieldLineCount(argumentsText, "content");
  if (content === null) return null;
  return { added: content, removed: 0 };
}

/**
 * 统计字段字符串值中已接收的行数。
 *
 * 与严格 JSON 解析不同：值未闭合时统计已到达的部分。
 * 以转义状态机扫描，`\n` 转义计行、字面反斜杠不误计。
 *
 * @param raw JSON 参数片段
 * @param key 字段名
 * @returns 已接收行数；字段尚未出现时返回 null，值为空串时返回 0
 */
function lenientFieldLineCount(raw: string, key: string): number | null {
  const pattern = `"${key}"`;
  const keyIndex = raw.indexOf(pattern);
  if (keyIndex < 0) return null;
  const afterKey = raw.slice(keyIndex + pattern.length);
  const colonIndex = afterKey.indexOf(":");
  if (colonIndex < 0) return null;
  const afterColon = afterKey.slice(colonIndex + 1).trimStart();
  if (!afterColon.startsWith("\"")) return null;
  const value = afterColon.slice(1);
  let newlines = 0;
  let hasContent = false;
  let escaped = false;
  for (const ch of value) {
    if (escaped) {
      if (ch === "n") newlines += 1;
      hasContent = true;
      escaped = false;
      continue;
    }
    if (ch === "\\") {
      escaped = true;
      continue;
    }
    if (ch === "\"") break;
    hasContent = true;
  }
  if (!hasContent) return 0;
  return newlines + 1;
}
