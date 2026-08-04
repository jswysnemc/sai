import type { SideBySideRow } from "./side-by-side";

/** 折叠未改动区时，上下各保留的行数 */
export const CONTEXT_MARGIN = 3;

/** 对齐行切分后的段：未改动上下文段或变更段 */
export type RowSegment =
  | { kind: "context"; rows: SideBySideRow[] }
  | { kind: "change"; rows: SideBySideRow[] };

/**
 * 判断一行是否为未改动的上下文行。
 *
 * 左右都存在且均为 context 才算上下文；任一侧缺失或为增删即变更。
 *
 * @param row 对齐行
 * @returns 上下文行为 true
 */
export function isContextRow(row: SideBySideRow): boolean {
  return Boolean(
    row.left && row.right && row.left.kind === "context" && row.right.kind === "context"
  );
}

/**
 * 把左右对齐的行序列切分为上下文段与变更段。
 *
 * hunk 与 no-newline 标记行归入上下文段，渲染时作为整行分隔条；
 * 它们不参与变更计数，也不影响折叠。
 *
 * @param rows 对齐行序列
 * @returns 交替的上下文段与变更段
 */
export function segmentRows(rows: SideBySideRow[]): RowSegment[] {
  const segments: RowSegment[] = [];
  let context: SideBySideRow[] = [];
  let change: SideBySideRow[] = [];

  /** 收尾上下文段。 */
  const flushContext = (): void => {
    if (context.length > 0) {
      segments.push({ kind: "context", rows: context });
      context = [];
    }
  };

  /** 收尾变更段。 */
  const flushChange = (): void => {
    if (change.length > 0) {
      segments.push({ kind: "change", rows: change });
      change = [];
    }
  };

  for (const row of rows) {
    // 1. 边界标记行归入上下文，作为整行分隔条
    if (row.left && (row.left.kind === "hunk" || row.left.kind === "no-newline")) {
      flushChange();
      context.push(row);
      continue;
    }
    if (isContextRow(row)) {
      // 上下文行必须切断未闭合的变更段，否则被上下文隔开的两个变更块会合并
      flushChange();
      context.push(row);
    } else {
      flushContext();
      change.push(row);
    }
  }
  flushChange();
  flushContext();
  return segments;
}

/**
 * 计算上下文段的折叠计划。
 *
 * 段长不超过两倍边距时不折叠；否则保留首尾各 CONTEXT_MARGIN 行，
 * 中间整体折叠。
 *
 * @param rowCount 上下文段行数
 * @returns 折叠计划；不折叠时 foldCount 为 0
 */
export function foldPlan(rowCount: number): { head: number; foldCount: number; tail: number } {
  if (rowCount <= CONTEXT_MARGIN * 2) {
    return { head: rowCount, foldCount: 0, tail: 0 };
  }
  return { head: CONTEXT_MARGIN, foldCount: rowCount - CONTEXT_MARGIN * 2, tail: CONTEXT_MARGIN };
}
