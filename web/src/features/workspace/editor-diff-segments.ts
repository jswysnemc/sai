import type { DiffLine } from "../chat/tool-renderers/diff/diff-model";

/** 折叠条里的一段：补丁自带的上下文行，或需要按行号回填的省略区间。 */
export type EditorDiffPiece =
  | { kind: "line"; line: DiffLine; index: number }
  | { kind: "fold"; line: DiffLine };

/** 编辑器差异的可视分段：连续改动，或中间被收起的未修改行。 */
export type EditorDiffSegment =
  | { kind: "change"; lines: Array<{ line: DiffLine; index: number }> }
  | { kind: "gap"; count: number; pieces: EditorDiffPiece[] };

/**
 * 把统一差异收成改动块和未修改间隔。
 *
 * 补丁里的上下文行与 hunk 省略区间会并进同一条间隔，
 * 这样相邻的未修改内容只出现一条折叠条。
 *
 * @param lines 解析后的差异行
 * @returns 按原文顺序排列的分段
 */
export function buildEditorDiffSegments(lines: DiffLine[]): EditorDiffSegment[] {
  const segments: EditorDiffSegment[] = [];
  let change: Array<{ line: DiffLine; index: number }> = [];
  let pieces: EditorDiffPiece[] = [];
  let count = 0;

  /** 写出当前改动块。 */
  const flushChange = () => {
    if (change.length === 0) return;
    segments.push({ kind: "change", lines: change });
    change = [];
  };

  /** 写出当前未修改间隔。 */
  const flushGap = () => {
    if (count === 0) return;
    segments.push({ kind: "gap", count, pieces });
    pieces = [];
    count = 0;
  };

  lines.forEach((line, index) => {
    if (line.kind === "added" || line.kind === "removed" || line.kind === "no-newline") {
      flushGap();
      change.push({ line, index });
      return;
    }
    flushChange();
    if (line.kind === "context") {
      pieces.push({ kind: "line", line, index });
      count += 1;
      return;
    }
    const folded = line.foldedCount ?? 0;
    if (line.kind === "hunk" && folded > 0) {
      pieces.push({ kind: "fold", line });
      count += folded;
    }
  });
  flushChange();
  flushGap();
  return segments;
}
