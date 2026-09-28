import type { ReactNode } from "react";

/** 数据表列定义。 */
export type DataColumn<Row> = {
  id: string;
  header: ReactNode;
  /** 数值列：右对齐并使用等宽数字 */
  numeric?: boolean;
  align?: "left" | "center" | "right";
  /** 列宽，例如 8rem */
  width?: string;
  render: (row: Row) => ReactNode;
  /** 提供时该列可排序 */
  sortValue?: (row: Row) => number | string;
};

/** 排序状态。 */
export type SortState = { id: string; direction: "asc" | "desc" } | null;

/**
 * 按排序状态排列数据行，不修改原数组。
 *
 * @param rows 数据行
 * @param columns 列定义
 * @param sort 排序状态；为空或列不可排序时保持原顺序
 * @returns 排序后的新数组
 */
export function sortRows<Row>(rows: readonly Row[], columns: readonly DataColumn<Row>[], sort: SortState): Row[] {
  const column = sort ? columns.find((item) => item.id === sort.id) : undefined;
  if (!sort || !column?.sortValue) return [...rows];
  const read = column.sortValue;
  const factor = sort.direction === "asc" ? 1 : -1;
  return [...rows].sort((left, right) => {
    const a = read(left);
    const b = read(right);
    // 1. 数值直接相减，字符串按当前语言排序
    if (typeof a === "number" && typeof b === "number") return (a - b) * factor;
    return String(a).localeCompare(String(b)) * factor;
  });
}

/**
 * 计算点击表头后的下一个排序状态：同列在降序、升序间切换，换列从降序开始。
 *
 * @param current 当前排序状态
 * @param id 被点击的列
 * @returns 新的排序状态
 */
export function nextSort(current: SortState, id: string): SortState {
  if (current?.id !== id) return { id, direction: "desc" };
  return { id, direction: current.direction === "desc" ? "asc" : "desc" };
}
