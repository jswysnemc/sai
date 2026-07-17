import { Plus, Search } from "lucide-react";
import { useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useI18n } from "../i18n/use-i18n";

export type ObjectListItem = {
  id: string;
  name: string;
  meta?: string;
  icon?: ReactNode;
  marked?: boolean;
};

type ObjectListPanelProps = {
  title: string;
  items: ObjectListItem[];
  selectedId: string;
  searchPlaceholder: string;
  addLabel?: string;
  topSlot?: ReactNode;
  headerSlot?: ReactNode;
  onSelect: (id: string) => void;
  onAdd?: () => void;
};

/**
 * 渲染对象列表中列，提供搜索过滤、数量统计、新增按钮和独立滚动。
 *
 * @param props 列表标题、条目、选中标识、插槽和操作回调
 * @returns 对象列表面板
 */
export function ObjectListPanel({ title, items, selectedId, searchPlaceholder, addLabel, topSlot, headerSlot, onSelect, onAdd }: ObjectListPanelProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const keyword = query.trim().toLowerCase();

  // 1. 按名称、标识和附注过滤条目
  const filtered = useMemo(
    () => items.filter((item) => !keyword
      || item.name.toLowerCase().includes(keyword)
      || item.id.toLowerCase().includes(keyword)
      || (item.meta ?? "").toLowerCase().includes(keyword)),
    [items, keyword]
  );

  return (
    <aside className="object-list" aria-label={title}>
      <div className="object-list-head">
        <span className="object-list-title">{title}<small>{items.length}</small></span>
        {onAdd && (
          <button type="button" className="object-list-add" onClick={onAdd} aria-label={addLabel ?? t("Add", "新增")} title={addLabel ?? t("Add", "新增")}>
            <Plus size={14} />
          </button>
        )}
      </div>
      {headerSlot}
      <label className="object-list-search">
        <Search size={13} />
        <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder={searchPlaceholder} spellCheck={false} />
      </label>
      {topSlot}
      <div className="object-list-scroll" role="list">
        {filtered.map((item) => (
          <button
            type="button"
            role="listitem"
            className={item.id === selectedId ? "object-list-item active" : "object-list-item"}
            key={item.id}
            title={item.name}
            onClick={() => onSelect(item.id)}
          >
            {item.icon && <span className="object-list-icon">{item.icon}</span>}
            <span className="object-list-copy"><strong>{item.name}</strong>{item.meta && <small>{item.meta}</small>}</span>
            {item.marked && <i className="object-list-mark" aria-hidden="true" />}
          </button>
        ))}
        {filtered.length === 0 && <div className="object-list-empty">{t("No matching items", "没有匹配的条目")}</div>}
      </div>
    </aside>
  );
}
