import { ChevronDown } from "lucide-react";
import type { KeyboardEvent, ReactNode } from "react";
import "./tool-card-shell.css";

/** 卡片色调，决定图标、导轨与状态标记的颜色 */
export type ToolCardTone = "neutral" | "running" | "success" | "danger" | "attention";

type ToolCardShellProps = {
  /** 色调 */
  tone: ToolCardTone;
  /** 左侧语义图标，表达"这是什么操作" */
  icon: ReactNode;
  /** 操作名称，如 Read / Shell / 执行命令 */
  title: string;
  /** 操作对象，如文件路径或命令，等宽字体单行截断 */
  target?: ReactNode;
  /** 对象的完整文本，挂在 title 属性上供悬停查看 */
  targetTitle?: string;
  /** 右侧次要信息，如耗时或退出码 */
  meta?: ReactNode;
  /** 状态标记，位于展开箭头左侧；不传则不占位 */
  status?: ReactNode;
  /** 是否展开详情 */
  expanded: boolean;
  /** 切换展开状态 */
  onToggle: () => void;
  /** 折叠体内容 */
  children?: ReactNode;
  /** 附加类名 */
  className?: string;
};

/**
 * 工具、权限与清单卡片共用的外壳。
 *
 * 统一三件事：头部的信息顺序（图标 → 名称 → 对象 → 元信息 → 状态 → 箭头）、
 * 状态的唯一表达位置（左侧图标与状态标记同色，不再用文字重复一遍），
 * 以及展开体的层级线索（一条与图标对齐的左导轨，不用外框）。
 *
 * @param props 卡片内容与展开控制
 * @returns 统一样式的可折叠卡片
 */
export function ToolCardShell({
  tone,
  icon,
  title,
  target,
  targetTitle,
  meta,
  status,
  expanded,
  onToggle,
  children,
  className
}: ToolCardShellProps) {
  /**
   * 在头部空白区域按下回车或空格时切换展开状态。
   *
   * @param event 头部键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target !== event.currentTarget) return;
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    onToggle();
  };

  const classes = ["tool-shell", `tone-${tone}`, expanded ? "is-open" : "", className ?? ""]
    .filter(Boolean)
    .join(" ");

  return (
    <section className={classes}>
      <div
        className="tool-shell-head"
        role="button"
        tabIndex={0}
        aria-expanded={expanded}
        onClick={onToggle}
        onKeyDown={handleKeyDown}
      >
        <span className="tool-shell-icon" aria-hidden>{icon}</span>
        <span className="tool-shell-title">{title}</span>
        {target ? (
          <span className="tool-shell-target" title={targetTitle}>{target}</span>
        ) : (
          <span className="tool-shell-target" aria-hidden />
        )}
        {meta ? <span className="tool-shell-meta">{meta}</span> : null}
        {status ? <span className="tool-shell-status" aria-hidden>{status}</span> : null}
        <ChevronDown size={14} className="tool-shell-chevron" aria-hidden />
      </div>
      {expanded && children ? <div className="tool-shell-body">{children}</div> : null}
    </section>
  );
}
