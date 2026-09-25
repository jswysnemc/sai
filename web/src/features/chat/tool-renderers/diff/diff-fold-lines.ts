import type { DiffLine } from "./diff-model";

/**
 * 用工作区文件正文补上 hunk 之间被省略的未修改行。
 *
 * 补丁本身不包含这些行，只能按解析时记下的新文件行号去当前文件里取。
 * 新旧两侧省略长度一致时，同时标上旧行号，方便并排视图对齐。
 *
 * @param content 工作区文件全文
 * @param line 带省略区间的 hunk 标记
 * @returns 可按上下文行渲染的差异行；区间无效时为空
 */
export function contextLinesFromFile(content: string, line: DiffLine): DiffLine[] {
  const start = line.foldStart;
  const end = line.foldEnd;
  if (!start || !end || end < start) return [];
  const rows = content.replace(/\r\n/g, "\n").replace(/\r/g, "\n").split("\n");
  const oldSpan = line.foldOldStart && line.foldOldEnd && line.foldOldEnd >= line.foldOldStart
    ? line.foldOldEnd - line.foldOldStart + 1
    : 0;
  const count = end - start + 1;
  const lines: DiffLine[] = [];
  for (let offset = 0; offset < count; offset += 1) {
    const newLine = start + offset;
    lines.push({
      kind: "context",
      text: rows[newLine - 1] ?? "",
      newLine,
      oldLine: oldSpan === count && line.foldOldStart ? line.foldOldStart + offset : undefined
    });
  }
  return lines;
}
