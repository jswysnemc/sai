import type { DiffLine } from "./diff-model";

/** 并排视图的一行：左右各占一格，缺失侧为空槽。 */
export type SideBySideRow = {
  left: DiffLine | null;
  right: DiffLine | null;
};

/**
 * 把统一差异行序列转换为左右对齐的行序列。
 *
 * 对齐规则与主流 diff 工具一致：
 * - context 行左右同列
 * - 连续的删除块与紧随其后的新增块按位置配对，同行左右对照
 * - 删除多于新增时，多出的删除行右侧留空；反之左侧留空
 * - 孤立的新增块左侧整块留空
 *
 * hunk 与 no-newline 标记不属于代码行，作为整行分隔保留在序列中。
 *
 * @param lines 已解析并标注字符级差异的行序列
 * @returns 左右对齐的行序列
 */
export function buildSideBySide(lines: DiffLine[]): SideBySideRow[] {
  const rows: SideBySideRow[] = [];
  let index = 0;
  while (index < lines.length) {
    const line = lines[index];
    // 1. 边界与标记行整行保留
    if (line.kind === "hunk" || line.kind === "no-newline") {
      rows.push({ left: line, right: null });
      index += 1;
      continue;
    }
    // 2. context 行左右同列
    if (line.kind === "context") {
      rows.push({ left: line, right: line });
      index += 1;
      continue;
    }
    // 3. 收集连续删除块
    if (line.kind === "removed") {
      const removedStart = index;
      while (index < lines.length && lines[index].kind === "removed") index += 1;
      const removed = lines.slice(removedStart, index);
      // 4. 紧随其后的连续新增块与之配对
      const addedStart = index;
      while (index < lines.length && lines[index].kind === "added") index += 1;
      const added = lines.slice(addedStart, index);
      const span = Math.max(removed.length, added.length);
      for (let offset = 0; offset < span; offset += 1) {
        rows.push({
          left: removed[offset] ?? null,
          right: added[offset] ?? null
        });
      }
      continue;
    }
    // 5. 孤立新增块：左侧留空
    const addedStart = index;
    while (index < lines.length && lines[index].kind === "added") index += 1;
    for (const addedLine of lines.slice(addedStart, index)) {
      rows.push({ left: null, right: addedLine });
    }
  }
  return rows;
}
