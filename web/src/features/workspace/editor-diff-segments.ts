import { CONTEXT_MARGIN } from "../chat/tool-renderers/diff/diff-blocks";
import type { DiffLine } from "../chat/tool-renderers/diff/diff-model";

/** 折叠条里的一段：补丁自带的上下文行，或需要按行号回填的省略区间。 */
export type EditorDiffPiece =
  | { kind: "line"; line: DiffLine; index: number }
  | { kind: "fold"; line: DiffLine };

/** 编辑器差异的可视分段：连续改动、紧贴改动的未修改行，或中间被收起的间隔。 */
export type EditorDiffSegment =
  | { kind: "change" | "context"; lines: Array<{ line: DiffLine; index: number }> }
  | { kind: "gap"; count: number; pieces: EditorDiffPiece[] };

type RawSegment =
  | { kind: "change"; lines: Array<{ line: DiffLine; index: number }> }
  | { kind: "gap"; pieces: EditorDiffPiece[] };

/**
 * 把统一差异收成改动块和未修改间隔。
 *
 * 紧贴改动的未修改行上下各留 3 行直接展示。更长的间隔仍收成一条折叠条，
 * 补丁里的上下文与 hunk 省略区间继续并在同一条里。
 *
 * @param lines 解析后的差异行
 * @returns 按原文顺序排列的分段
 */
export function buildEditorDiffSegments(lines: DiffLine[]): EditorDiffSegment[] {
  const raw: RawSegment[] = [];
  let change: Array<{ line: DiffLine; index: number }> = [];
  let pieces: EditorDiffPiece[] = [];

  /** 写出当前改动块。 */
  const flushChange = () => {
    if (change.length === 0) return;
    raw.push({ kind: "change", lines: change });
    change = [];
  };

  /** 写出当前未修改间隔。 */
  const flushGap = () => {
    if (pieces.length === 0) return;
    raw.push({ kind: "gap", pieces });
    pieces = [];
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
      return;
    }
    const folded = line.foldedCount ?? 0;
    if (line.kind === "hunk" && folded > 0) pieces.push({ kind: "fold", line });
  });
  flushChange();
  flushGap();
  return raw.flatMap((segment, index) => {
    if (segment.kind === "change") return [segment];
    const before = raw.slice(0, index).some((item) => item.kind === "change");
    const after = raw.slice(index + 1).some((item) => item.kind === "change");
    const position = before && after ? "middle" : before ? "trailing" : "leading";
    return exposeGap(segment.pieces, position);
  });
}

/**
 * 从间隔两端剥出紧贴改动的未修改行，其余留在折叠条里。
 *
 * 文件开头只保留改动上方的行，文件结尾只保留改动下方的行。
 * 两端合计不超过两倍边距时整段展开。
 *
 * @param pieces 间隔内的上下文行和省略区间
 * @param position 间隔相对改动的位置
 * @returns 可见上下文和剩余折叠条
 */
function exposeGap(pieces: EditorDiffPiece[], position: "leading" | "middle" | "trailing"): EditorDiffSegment[] {
  const total = pieces.reduce((sum, piece) => sum + pieceUnits(piece), 0);
  if (total <= CONTEXT_MARGIN * 2 && pieces.every((piece) => piece.kind === "line")) {
    return [{ kind: "context", lines: pieces.flatMap((piece) => piece.kind === "line" ? [piece] : []) }];
  }
  const headBudget = position === "leading" ? 0 : CONTEXT_MARGIN;
  const tailBudget = position === "trailing" ? 0 : CONTEXT_MARGIN;
  const head = takeEdgeLines(pieces, headBudget, "start");
  const tail = takeEdgeLines(head.rest, tailBudget, "end");
  const middleCount = tail.rest.reduce((sum, piece) => sum + pieceUnits(piece), 0);
  if (middleCount === 0) {
    const lines = [...head.lines, ...tail.lines];
    return lines.length > 0 ? [{ kind: "context", lines }] : [];
  }
  const segments: EditorDiffSegment[] = [];
  if (head.lines.length > 0) segments.push({ kind: "context", lines: head.lines });
  segments.push({ kind: "gap", count: middleCount, pieces: tail.rest });
  if (tail.lines.length > 0) segments.push({ kind: "context", lines: tail.lines });
  return segments;
}

/**
 * 计算一段间隔占用的行数。
 *
 * @param piece 上下文行或省略区间
 * @returns 行数
 */
function pieceUnits(piece: EditorDiffPiece): number {
  return piece.kind === "line" ? 1 : piece.line.foldedCount ?? 0;
}

/**
 * 从间隔一端连续取出未修改行，遇到省略区间即停止。
 *
 * @param pieces 剩余间隔
 * @param budget 最多取出的行数
 * @param edge 从开头还是结尾取
 * @returns 取出的行和未动过的剩余片段
 */
function takeEdgeLines(pieces: EditorDiffPiece[], budget: number, edge: "start" | "end"): {
  lines: Array<{ line: DiffLine; index: number }>;
  rest: EditorDiffPiece[];
} {
  if (budget <= 0 || pieces.length === 0) return { lines: [], rest: pieces };
  const ordered = edge === "start" ? pieces : [...pieces].reverse();
  const lines: Array<{ line: DiffLine; index: number }> = [];
  for (const piece of ordered) {
    if (lines.length >= budget || piece.kind !== "line") break;
    lines.push(piece);
  }
  if (edge === "end") lines.reverse();
  const rest = edge === "start" ? pieces.slice(lines.length) : pieces.slice(0, pieces.length - lines.length);
  return { lines, rest };
}
