import type { RowSegment } from "./diff-blocks";
import type { SideBySideRow } from "./side-by-side";

/** 变更块的视觉色调。 */
export type DiffChangeTone = "added" | "removed" | "mixed" | "neutral";

/**
 * 获取差异段中所有变更段的索引。
 *
 * @param segments 已经按上下文与变更切分的差异段
 * @returns 变更段在原数组中的索引
 */
export function changeSegmentIndexes(segments: RowSegment[]): number[] {
  return segments.reduce<number[]>((indexes, segment, index) => {
    if (segment.kind === "change") indexes.push(index);
    return indexes;
  }, []);
}

/**
 * 将变更序号限制在当前文件的有效范围内。
 *
 * @param ordinal 目标变更序号，从零开始
 * @param changeCount 当前文件的变更块数量
 * @returns 可安全用于数组访问的变更序号
 */
export function clampChangeOrdinal(ordinal: number, changeCount: number): number {
  if (changeCount <= 0) return 0;
  return Math.min(Math.max(ordinal, 0), changeCount - 1);
}

/**
 * 根据一个变更块的左右内容确定连接带色调。
 *
 * @param rows 变更块中按位置配对的左右行
 * @returns 增加、删除、混合或中性连接带色调
 */
export function changeTone(rows: SideBySideRow[]): DiffChangeTone {
  const hasRemoved = rows.some((row) => row.left?.kind === "removed");
  const hasAdded = rows.some((row) => row.right?.kind === "added");
  if (hasRemoved && hasAdded) return "mixed";
  if (hasAdded) return "added";
  if (hasRemoved) return "removed";
  return "neutral";
}

/**
 * 根据一对左右行确定单行连接器的色调。
 *
 * @param row 左右对齐的一行
 * @returns 增加、删除、混合或中性连接带色调
 */
export function changeRowTone(row: SideBySideRow): DiffChangeTone {
  const leftKind = row.left?.kind;
  const rightKind = row.right?.kind;
  if (leftKind === "removed" && rightKind === "added") return "mixed";
  if (rightKind === "added") return "added";
  if (leftKind === "removed") return "removed";
  return "neutral";
}
