import { ChevronRight } from "lucide-react";
import { useId, type ReactNode } from "react";

type SidebarPurposeSectionProps = {
  title: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  action?: ReactNode;
  children: ReactNode;
};

/**
 * 渲染左栏可折叠分区，对齐 ZCode 的 purpose section。
 *
 * 标题行本身切换展开，右侧动作在悬停或键盘聚焦时出现，避免和折叠点击抢同一块热区。
 *
 * @param props 标题、展开状态、可选动作和分区内容
 * @returns 可折叠分区
 */
export function SidebarPurposeSection({ title, open, onOpenChange, action, children }: SidebarPurposeSectionProps) {
  const contentId = useId();
  return (
    <section className="sidebar-purpose-section group/purpose" aria-label={title}>
      <div className="sidebar-purpose-heading">
        <button
          type="button"
          className="sidebar-purpose-toggle"
          aria-expanded={open}
          aria-controls={contentId}
          onClick={() => onOpenChange(!open)}
        >
          <span>{title}</span>
          <ChevronRight size={14} className={open ? "is-open" : ""} aria-hidden="true" />
        </button>
        {action && <div className="sidebar-purpose-action">{action}</div>}
      </div>
      <div id={contentId} hidden={!open} className="sidebar-purpose-body">{children}</div>
    </section>
  );
}
