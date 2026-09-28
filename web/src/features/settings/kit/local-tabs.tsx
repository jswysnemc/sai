import type { ReactNode } from "react";
import { cx } from "./class-names";
import "./detail-header.css";

export type TabItem<T extends string> = {
  id: T;
  label: string;
  icon?: ReactNode;
  /** 页签后的计数 */
  count?: number;
};

type LocalTabsProps<T extends string> = {
  value: T;
  items: TabItem<T>[];
  onChange: (value: T) => void;
  ariaLabel: string;
  className?: string;
};

/**
 * 渲染组件内页签（不改变路由），样式与路由子页签一致。
 *
 * @param props 当前页签、页签列表、切换回调与可访问名称
 * @returns 页签栏
 */
export function LocalTabs<T extends string>({ value, items, onChange, ariaLabel, className }: LocalTabsProps<T>) {
  return (
    <div className={cx("sk-tabs", className)} role="tablist" aria-label={ariaLabel}>
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          role="tab"
          aria-selected={item.id === value}
          className={item.id === value ? "active" : undefined}
          onClick={() => onChange(item.id)}
        >
          {item.icon}
          {item.label}
          {item.count !== undefined && <em>{item.count}</em>}
        </button>
      ))}
    </div>
  );
}
