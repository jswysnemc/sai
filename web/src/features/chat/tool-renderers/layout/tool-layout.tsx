import { ChevronRight } from "lucide-react";
import { type KeyboardEvent, type ReactNode } from "react";
import { ToolDiffBadge } from "./tool-diff-badge";
import { ToolSummaryText } from "./tool-summary-text";
import "./tool-layout.css";
import "../../message/activity-stream.css";

type ToolLayoutProps = {
  /** 工具图标 */
  icon?: ReactNode;
  /** 类型标签，如"运行中"/"已运行" */
  kindLabel: string;
  /** 类型细节，紧随类型标签，可着色 */
  kindDetail?: ReactNode;
  /** 来源徽章，如子智能体名称 */
  sourceLabel?: string;
  /** 主文本，单行截断 */
  primaryText?: string;
  /** 主内容节点，用于文件链接等可交互对象；提供时覆盖 primaryText */
  primaryContent?: ReactNode;
  /** 副文本，等宽字体单行截断 */
  secondaryText?: string;
  /** 摘要文本切换动画的帧标识 */
  summaryContentKey?: string;
  /** 是否启用摘要切换动画 */
  animateSummary?: boolean;
  /** diff 增删行数 */
  diffCount?: { added: number; removed: number };
  /** 是否对 diff 数字做滚动逼近 */
  animateDiffCount?: boolean;
  /** diff 数字是否仍在流式增长 */
  diffCountActive?: boolean;
  /** 展开后是否隐藏 diff 徽章 */
  hideDiffCountWhenOpen?: boolean;
  /** 摘要行次级操作，常态隐藏，悬停或聚焦时出现 */
  actions?: ReactNode;
  /** 状态文字 */
  statusLabel?: ReactNode;
  /** 并行批次序号，紧贴摘要，如 4/10 */
  batchLabel?: string;
  /** 状态是否按失败着色 */
  showFailureStatus?: boolean;
  /** 是否正在运行，运行时类型标签走流光渐变 */
  isRunning?: boolean;
  /** 悬停提示的完整文本 */
  title?: string;
  /** 是否可折叠 */
  canToggle?: boolean;
  /** 是否展开 */
  expanded?: boolean;
  /** 切换展开状态 */
  onToggle?: () => void;
  /** 展开区内容 */
  children?: ReactNode;
};

/**
 * 工具调用卡片的统一外壳。
 *
 * 摘要行按"这是什么操作 → 操作对象 → 结果如何"排列，一行读完即可判断
 * 是否需要展开；展开箭头常态隐藏，悬停或运行时才出现，避免静止界面
 * 被一列箭头切碎。运行中类型标签走流光渐变，不额外占用状态位。
 *
 * @param props 卡片内容与折叠控制
 * @returns 可折叠的工具卡片
 */
export function ToolLayout({
  icon,
  kindLabel,
  kindDetail,
  sourceLabel,
  primaryText = "",
  primaryContent,
  secondaryText = "",
  summaryContentKey,
  animateSummary = false,
  diffCount,
  animateDiffCount = false,
  diffCountActive = false,
  hideDiffCountWhenOpen = false,
  actions,
  statusLabel,
  batchLabel,
  showFailureStatus = false,
  isRunning = false,
  title,
  canToggle = true,
  expanded = false,
  onToggle,
  children
}: ToolLayoutProps) {
  const interactive = canToggle && Boolean(onToggle);
  const showDiff = diffCount && !(expanded && hideDiffCountWhenOpen);
  const frameKey = summaryContentKey ?? `${primaryText}:${secondaryText}:${String(statusLabel ?? "")}`;

  /**
   * 在摘要行上按下回车或空格时切换展开状态。
   *
   * @param event 键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    onToggle?.();
  };

  return (
    <section className="group/tool tool-layout min-w-0">
      <div
        className={[
          "tool-layout-head",
          interactive ? "is-interactive" : ""
        ].join(" ")}
        role={interactive ? "button" : undefined}
        tabIndex={interactive ? 0 : undefined}
        aria-expanded={interactive ? expanded : undefined}
        onClick={interactive ? onToggle : undefined}
        onKeyDown={interactive ? handleKeyDown : undefined}
        title={title}
      >
        {icon ? (
          <span className="tool-layout-icon" aria-hidden>
            {icon}
          </span>
        ) : null}
        <span
          className={[
            "shrink-0 whitespace-nowrap font-medium",
            isRunning ? "animated-gradient-text" : "text-ink-soft"
          ].join(" ")}
        >
          {kindLabel}
        </span>
        {kindDetail ? <span className="shrink-0 whitespace-nowrap">{kindDetail}</span> : null}
        {sourceLabel ? (
          <span className="shrink-0 rounded border border-border bg-background-alt px-1.5 py-0.5 text-ui-xs text-ink-soft">
            {sourceLabel}
          </span>
        ) : null}
        {primaryContent ? (
          <span className="tool-layout-summary">
            <span className="min-w-0 truncate">{primaryContent}</span>
          </span>
        ) : (
          <ToolSummaryText
            contentKey={frameKey}
            primaryText={primaryText}
            secondaryText={secondaryText}
            animate={animateSummary}
          />
        )}
        <span className="tool-layout-meta">
          {actions}
          {showDiff ? (
            <ToolDiffBadge
              added={diffCount.added}
              removed={diffCount.removed}
              animate={animateDiffCount}
              active={diffCountActive}
            />
          ) : null}
          {statusLabel ? (
            <span className={showFailureStatus ? "text-destructive" : "text-ink-soft"}>{statusLabel}</span>
          ) : null}
          {batchLabel ? <span className="tool-layout-batch">{batchLabel}</span> : null}
          {interactive ? (
            <ChevronRight
              size={14}
              aria-hidden
              className={["tool-layout-chevron", expanded ? "is-open" : ""].join(" ")}
            />
          ) : null}
        </span>
      </div>
      {expanded && children ? (
        <div className="tool-layout-body outline-none">{children}</div>
      ) : null}
    </section>
  );
}
