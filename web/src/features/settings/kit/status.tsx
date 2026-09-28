import type { ReactNode } from "react";
import { CircleAlert, Info, TriangleAlert, CircleCheck } from "../../../shared/ui/icons";
import { cx } from "./class-names";
import "./status.css";

/** 状态色调。 */
export type StatusTone = "neutral" | "success" | "warning" | "danger" | "info";

type StatusBadgeProps = {
  tone?: StatusTone;
  /** 前置圆点，用于运行状态 */
  dot?: boolean;
  /** 等宽字体，用于标识与数值 */
  mono?: boolean;
  title?: string;
  children: ReactNode;
};

/**
 * 渲染状态徽标。
 *
 * @param props 色调、圆点、字体与内容
 * @returns 徽标
 */
export function StatusBadge({ tone = "neutral", dot, mono, title, children }: StatusBadgeProps) {
  return (
    <span className={cx("sk-badge", `tone-${tone}`, mono && "is-mono")} title={title}>
      {dot && <i aria-hidden="true" />}
      {children}
    </span>
  );
}

type InlineNoticeProps = {
  tone?: Exclude<StatusTone, "neutral"> | "neutral";
  /** 右侧操作，例如重试按钮 */
  action?: ReactNode;
  className?: string;
  children: ReactNode;
};

/**
 * 渲染单行内联提示，用于错误、警告与说明。
 *
 * @param props 色调、操作与内容
 * @returns 内联提示条
 */
export function InlineNotice({ tone = "neutral", action, className, children }: InlineNoticeProps) {
  const Icon = tone === "danger" ? CircleAlert : tone === "warning" ? TriangleAlert : tone === "success" ? CircleCheck : Info;
  return (
    <div className={cx("sk-notice", `tone-${tone}`, className)} role={tone === "danger" ? "alert" : "status"}>
      <Icon size={14} />
      <div className="sk-notice-body">{children}</div>
      {action && <div className="sk-notice-action">{action}</div>}
    </div>
  );
}
