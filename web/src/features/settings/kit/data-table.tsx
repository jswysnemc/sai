import { useMemo, useState, type ReactNode } from "react";
import { ArrowDown, ArrowUp } from "../../../shared/ui/icons";
import { useI18n } from "../../i18n/use-i18n";
import { cx } from "./class-names";
import { nextSort, sortRows, type DataColumn, type SortState } from "./data-table-state";
import "./data-table.css";

export type { DataColumn, SortState } from "./data-table-state";

/** 批量选择配置。 */
export type TableSelection = {
  selected: ReadonlySet<string>;
  onChange: (next: Set<string>) => void;
};

type DataTableProps<Row> = {
  columns: DataColumn<Row>[];
  rows: readonly Row[];
  rowKey: (row: Row) => string;
  /** 行点击回调；提供时行可点击 */
  onRowClick?: (row: Row) => void;
  /** 无数据时的提示 */
  empty?: ReactNode;
  initialSort?: SortState;
  /** 批量选择；提供时首列为复选框 */
  selection?: TableSelection;
  /** 表格的可访问名称 */
  label: string;
  className?: string;
};

/**
 * 渲染紧凑数据表：表头吸顶、数值右对齐、可排序、可批量选择。
 *
 * @param props 列定义、数据行、行键、点击与选择回调
 * @returns 数据表
 */
export function DataTable<Row>({
  columns,
  rows,
  rowKey,
  onRowClick,
  empty,
  initialSort = null,
  selection,
  label,
  className
}: DataTableProps<Row>) {
  const { t } = useI18n();
  const [sort, setSort] = useState<SortState>(initialSort);
  const sorted = useMemo(() => sortRows(rows, columns, sort), [rows, columns, sort]);
  const keys = sorted.map(rowKey);
  const allSelected = Boolean(selection) && keys.length > 0 && keys.every((key) => selection?.selected.has(key));

  /**
   * 切换单行选中状态。
   *
   * @param key 行键
   * @returns 无返回值
   */
  const toggleRow = (key: string) => {
    if (!selection) return;
    const next = new Set(selection.selected);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    selection.onChange(next);
  };

  /**
   * 全选或取消全选当前可见行。
   *
   * @returns 无返回值
   */
  const toggleAll = () => {
    if (!selection) return;
    selection.onChange(allSelected ? new Set() : new Set(keys));
  };

  return (
    <div className={cx("sk-table-wrap", className)}>
      <table className="sk-table" aria-label={label}>
        <thead>
          <tr>
            {selection && (
              <th className="is-check">
                <input type="checkbox" checked={allSelected} onChange={toggleAll} aria-label={t("Select all", "全选")} />
              </th>
            )}
            {columns.map((column) => (
              <th
                key={column.id}
                className={cx(column.numeric && "is-num", column.align === "center" && "is-center")}
                style={column.width ? { width: column.width } : undefined}
                aria-sort={sort?.id === column.id ? (sort.direction === "asc" ? "ascending" : "descending") : undefined}
              >
                {column.sortValue ? (
                  <button
                    type="button"
                    className={cx("sk-table-sort", sort?.id === column.id && "is-active")}
                    onClick={() => setSort((current) => nextSort(current, column.id))}
                  >
                    {column.header}
                    {sort?.id === column.id && (sort.direction === "asc" ? <ArrowUp size={12} /> : <ArrowDown size={12} />)}
                  </button>
                ) : column.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {sorted.length === 0 && (
            <tr className="sk-table-empty">
              <td colSpan={columns.length + (selection ? 1 : 0)}>{empty ?? t("No data", "暂无数据")}</td>
            </tr>
          )}
          {sorted.map((row, index) => {
            const key = keys[index];
            const selected = selection?.selected.has(key) ?? false;
            return (
              <tr
                key={key}
                className={cx(onRowClick && "is-clickable", selected && "is-selected")}
                onClick={onRowClick ? () => onRowClick(row) : undefined}
              >
                {selection && (
                  <td className="is-check" onClick={(event) => event.stopPropagation()}>
                    <input type="checkbox" checked={selected} onChange={() => toggleRow(key)} aria-label={t("Select row", "选择此行")} />
                  </td>
                )}
                {columns.map((column) => (
                  <td key={column.id} className={cx(column.numeric && "is-num", column.align === "center" && "is-center")}>
                    {column.render(row)}
                  </td>
                ))}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

type ShareBarProps = {
  /** 0 到 1 之间的占比 */
  ratio: number;
  /** 条形旁的数值文字 */
  label: ReactNode;
};

/**
 * 渲染数值旁的占比条。
 *
 * @param props 占比与数值文字
 * @returns 占比条
 */
export function ShareBar({ ratio, label }: ShareBarProps) {
  const width = `${Math.round(Math.min(1, Math.max(0, ratio)) * 100)}%`;
  return (
    <span className="sk-share">
      <span>{label}</span>
      <span className="sk-share-track" aria-hidden="true"><i style={{ width }} /></span>
    </span>
  );
}
