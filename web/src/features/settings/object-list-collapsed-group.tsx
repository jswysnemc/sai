import { ChevronRight } from "lucide-react";
import { useState } from "react";
import { ObjectListRow, type ObjectListItem } from "./object-list-row";

type ObjectListCollapsedGroupProps = {
  /** 分区标题 */
  title: string;
  /** 分区内条目 */
  items: ObjectListItem[];
  /** 当前选中标识 */
  selectedId: string;
  /** 选中回调 */
  onSelect: (id: string) => void;
};

/**
 * 渲染一个默认折叠的列表分区。
 *
 * 停用项既不该混在常用列表里干扰选择，也不能彻底藏起来——否则没有
 * 改回启用的入口。折叠分区同时满足这两点：标题行常驻并显示数量，
 * 内容按需展开。
 *
 * @param props 分区标题、条目、选中标识与回调
 * @returns 可折叠的列表分区；无条目时不渲染
 */
export function ObjectListCollapsedGroup({
  title,
  items,
  selectedId,
  onSelect
}: ObjectListCollapsedGroupProps) {
  // 选中项落在分区内时默认展开，否则用户看不到自己正在编辑的条目
  const [expanded, setExpanded] = useState(() =>
    items.some((item) => item.id === selectedId)
  );
  if (items.length === 0) return null;
  return (
    <div className={expanded ? "object-list-group expanded" : "object-list-group"}>
      <button
        type="button"
        className="object-list-group-head"
        onClick={() => setExpanded((value) => !value)}
        aria-expanded={expanded}
      >
        <ChevronRight size={13} aria-hidden />
        <span>{title}</span>
        <small>{items.length}</small>
      </button>
      {expanded && items.map((item) => (
        <ObjectListRow
          key={item.id}
          item={item}
          selected={item.id === selectedId}
          onSelect={onSelect}
        />
      ))}
    </div>
  );
}
