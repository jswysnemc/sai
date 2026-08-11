import type { ReactNode } from "react";

export type ObjectListItem = {
  id: string;
  name: string;
  meta?: string;
  icon?: ReactNode;
  marked?: boolean;
  /** 停用项：置灰展示，仍可选中以便改回启用 */
  muted?: boolean;
};

type ObjectListRowProps = {
  /** 行数据 */
  item: ObjectListItem;
  /** 是否为当前选中项 */
  selected: boolean;
  /** 选中回调 */
  onSelect: (id: string) => void;
};

/**
 * 渲染对象列表中的单行。
 *
 * 常规区与折叠区共用同一行结构，避免两处样式各自漂移。
 *
 * @param props 行数据、选中态与回调
 * @returns 列表行按钮
 */
export function ObjectListRow({ item, selected, onSelect }: ObjectListRowProps) {
  const className = [
    "object-list-item",
    selected ? "active" : "",
    item.muted ? "muted" : ""
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <button
      type="button"
      role="listitem"
      className={className}
      title={item.name}
      onClick={() => onSelect(item.id)}
    >
      {item.icon && <span className="object-list-icon">{item.icon}</span>}
      <span className="object-list-copy">
        <strong>{item.name}</strong>
        {item.meta && <small>{item.meta}</small>}
      </span>
      {item.marked && <i className="object-list-mark" aria-hidden="true" />}
    </button>
  );
}
