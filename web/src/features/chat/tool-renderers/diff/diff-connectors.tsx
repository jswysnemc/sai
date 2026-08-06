import type { SideBySideRow } from "./side-by-side";
import { changeRowTone } from "./diff-change-blocks";

/**
 * 渲染左右差异行之间的关系连接器。
 *
 * @param props 左右对齐行
 * @returns 中央关系连接器
 */
export function DiffConnector({ row }: { row: SideBySideRow }) {
  const tone = changeRowTone(row);
  return (
    <span
      className={`diff-idea-band diff-idea-connector diff-idea-connector-${tone}`}
      aria-hidden="true"
    />
  );
}
