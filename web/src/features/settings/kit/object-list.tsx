import { useMemo, useState, type ReactNode } from "react";
import { ChevronRight, Plus, Search } from "../../../shared/ui/icons";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { cx } from "./class-names";
import { SkSelect } from "./select-input";
import "./master-detail.css";

/** 对象列表条目。 */
export type ObjectListItem = {
  id: string;
  name: string;
  /** 次要信息，例如默认模型；自动换行 */
  meta?: string;
  /** 次要信息使用等宽字体 */
  metaMono?: boolean;
  /** 小标签，例如「内置」「已注册」 */
  tags?: string[];
  icon?: ReactNode;
  /** 行尾圆点，表示当前使用或已启用 */
  marked?: boolean;
  /** 弱化显示，例如已停用 */
  muted?: boolean;
};

type ObjectListProps = {
  title: string;
  items: ObjectListItem[];
  selectedId: string;
  onSelect: (id: string) => void;
  onAdd?: () => void;
  addLabel?: string;
  searchPlaceholder?: string;
  /** 折叠到独立分组的条目，例如已停用的供应商 */
  collapsedItems?: ObjectListItem[];
  collapsedTitle?: string;
  /** 列表上方的附加内容，例如总开关 */
  headerSlot?: ReactNode;
};

/**
 * 按名称、标识、次要信息与标签过滤条目。
 *
 * @param items 待过滤条目
 * @param keyword 小写关键词；为空时原样返回
 * @returns 命中的条目
 */
export function filterObjectItems(items: readonly ObjectListItem[], keyword: string): ObjectListItem[] {
  if (!keyword) return [...items];
  return items.filter((item) => [item.name, item.id, item.meta ?? "", ...(item.tags ?? [])]
    .some((value) => value.toLowerCase().includes(keyword)));
}

/**
 * 渲染对象列表：搜索、计数、新增与可折叠分组；窄容器下改为顶部选择器。
 *
 * @param props 标题、条目、选中项、折叠分组与操作回调
 * @returns 对象列表导航
 */
export function ObjectList({
  title,
  items,
  selectedId,
  onSelect,
  onAdd,
  addLabel,
  searchPlaceholder,
  collapsedItems = [],
  collapsedTitle,
  headerSlot
}: ObjectListProps) {
  const { t } = useI18n();
  const [query, setQuery] = useState("");
  const selectedCollapsed = collapsedItems.some((item) => item.id === selectedId);
  const [groupOpen, setGroupOpen] = useState(selectedCollapsed);
  const keyword = query.trim().toLowerCase();
  const visible = useMemo(() => filterObjectItems(items, keyword), [items, keyword]);
  const visibleCollapsed = useMemo(() => filterObjectItems(collapsedItems, keyword), [collapsedItems, keyword]);
  // 1. 搜索时自动展开折叠分组，避免命中项被收起
  const collapsedOpen = groupOpen || Boolean(keyword) || selectedCollapsed;
  const total = items.length + collapsedItems.length;
  const add = addLabel ?? t("Add", "新增");
  const pickerOptions = [...items, ...collapsedItems].map((item) => ({
    value: item.id,
    label: item.name,
    description: item.meta,
    icon: item.icon
  }));

  return (
    <div className="sk-object-nav">
      {headerSlot && <div className="sk-object-slot">{headerSlot}</div>}
      <nav className="sk-object-list" aria-label={title}>
        <div className="sk-object-list-head">
          <span className="sk-object-list-title">{title}<small>{total}</small></span>
          {onAdd && (
            <Button variant="ghost" size="icon" onClick={onAdd} aria-label={add} title={add}>
              <Plus size={14} />
            </Button>
          )}
        </div>
        <label className="sk-object-search">
          <Search size={14} />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder={searchPlaceholder ?? t("Filter", "筛选")}
            aria-label={searchPlaceholder ?? t("Filter", "筛选")}
            spellCheck={false}
          />
        </label>
        <div className="sk-object-rows">
          {visible.map((item) => (
            <ObjectRow key={item.id} item={item} selected={item.id === selectedId} onSelect={onSelect} />
          ))}
          {visibleCollapsed.length > 0 && (
            <>
              <button
                type="button"
                className="sk-object-group-toggle"
                aria-expanded={collapsedOpen}
                onClick={() => setGroupOpen((value) => !value)}
              >
                <ChevronRight size={12} />
                <span>{collapsedTitle ?? t("Disabled", "已停用")}</span>
                <small>{visibleCollapsed.length}</small>
              </button>
              {collapsedOpen && visibleCollapsed.map((item) => (
                <ObjectRow key={item.id} item={item} selected={item.id === selectedId} onSelect={onSelect} />
              ))}
            </>
          )}
          {visible.length === 0 && visibleCollapsed.length === 0 && (
            <div className="sk-object-empty">
              {total === 0 ? t("Nothing here yet", "暂无条目") : t("No matching items", "没有匹配的条目")}
            </div>
          )}
        </div>
      </nav>
      {total > 0 && (
        <div className="sk-object-picker">
          <SkSelect
            value={selectedId}
            options={pickerOptions}
            onChange={onSelect}
            ariaLabel={title}
            menuPreferredWidth={320}
          />
          {onAdd && (
            <Button variant="secondary" size="icon" onClick={onAdd} aria-label={add} title={add}>
              <Plus size={14} />
            </Button>
          )}
        </div>
      )}
    </div>
  );
}

type ObjectRowProps = {
  item: ObjectListItem;
  selected: boolean;
  onSelect: (id: string) => void;
};

/**
 * 渲染单个对象条目：图标、名称、状态圆点、次要信息与标签。
 *
 * @param props 条目数据、是否选中与选择回调
 * @returns 可点击的条目行
 */
function ObjectRow({ item, selected, onSelect }: ObjectRowProps) {
  return (
    <button
      type="button"
      className={cx("sk-object-row", item.muted && "is-muted")}
      aria-current={selected || undefined}
      onClick={() => onSelect(item.id)}
    >
      <span className="sk-object-row-icon">{item.icon}</span>
      <span className="sk-object-row-name">{item.name}</span>
      {item.marked ? <span className="sk-object-row-mark" aria-hidden="true" /> : <span />}
      {item.meta && <span className={cx("sk-object-row-meta", item.metaMono && "is-mono")}>{item.meta}</span>}
      {item.tags && item.tags.length > 0 && (
        <span className="sk-object-row-tags">
          {item.tags.map((tag) => <span className="sk-object-tag" key={tag}>{tag}</span>)}
        </span>
      )}
    </button>
  );
}
