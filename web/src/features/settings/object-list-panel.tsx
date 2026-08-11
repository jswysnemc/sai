import { Plus, Search } from "lucide-react";
import { useMemo, useState } from "react";
import type { ReactNode } from "react";
import { useI18n } from "../i18n/use-i18n";
import { ObjectListCollapsedGroup } from "./object-list-collapsed-group";
import { ObjectListRow, type ObjectListItem } from "./object-list-row";
import "./object-list-panel.css";

export type { ObjectListItem } from "./object-list-row";

type ObjectListPanelProps = {
  title: string;
  items: ObjectListItem[];
  selectedId: string;
  searchPlaceholder: string;
  addLabel?: string;
  topSlot?: ReactNode;
  headerSlot?: ReactNode;
  /** 折叠到独立分区的条目，例如已停用的供应商 */
  collapsedItems?: ObjectListItem[];
  /** 折叠分区标题 */
  collapsedTitle?: string;
  onSelect: (id: string) => void;
  onAdd?: () => void;
};

/**
 * 渲染对象列表中列，提供搜索过滤、数量统计、新增按钮和独立滚动。
 *
 * @param props 列表标题、条目、选中标识、插槽和操作回调
 * @returns 对象列表面板
 */
export function ObjectListPanel({
  title,
  items,
  selectedId,
  searchPlaceholder,
  addLabel,
  topSlot,
  headerSlot,
  collapsedItems,
  collapsedTitle,
  onSelect,
  onAdd
}: ObjectListPanelProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const keyword = query.trim().toLowerCase();

  /**
   * 按名称、标识和附注过滤条目。
   *
   * @param list 待过滤条目
   * @returns 命中关键词的条目
   */
  const matching = (list: readonly ObjectListItem[]) => list.filter((item) => !keyword
    || item.name.toLowerCase().includes(keyword)
    || item.id.toLowerCase().includes(keyword)
    || (item.meta ?? "").toLowerCase().includes(keyword));

  // 1. 常规区与折叠区各自过滤，搜索时两边都要能命中
  const filtered = useMemo(() => matching(items), [items, keyword]);
  const filteredCollapsed = useMemo(
    () => matching(collapsedItems ?? []),
    [collapsedItems, keyword]
  );
  // 2. 计数含折叠项：折叠只是收起，不代表条目不存在
  const total = items.length + (collapsedItems?.length ?? 0);

  return (
    <aside className="object-list" aria-label={title}>
      <div className="object-list-head">
        <span className="object-list-title">{title}<small>{total}</small></span>
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
          <ObjectListRow
            key={item.id}
            item={item}
            selected={item.id === selectedId}
            onSelect={onSelect}
          />
        ))}
        {filteredCollapsed.length > 0 && (
          <ObjectListCollapsedGroup
            title={collapsedTitle ?? t("Disabled", "已停用")}
            items={filteredCollapsed}
            selectedId={selectedId}
            onSelect={onSelect}
          />
        )}
        {filtered.length === 0 && filteredCollapsed.length === 0 && (
          <div className="object-list-empty">{t("No matching items", "没有匹配的条目")}</div>
        )}
      </div>
    </aside>
  );
}
