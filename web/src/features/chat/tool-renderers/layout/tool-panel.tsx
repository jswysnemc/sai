import type { ReactNode } from "react";
import "./tool-panel.css";

type ToolPanelProps = {
  /** 面板内容 */
  children: ReactNode;
  /** 附加类名 */
  className?: string;
};

/**
 * 工具卡展开区的轻量内容面板。
 *
 * 只保留一层细边框与浅底，不再额外塞大块内边距——内边距由各工具视图自己控制，
 * 避免 Read/Shell 再套一层「框中框」。
 *
 * @param props 面板内容与附加类名
 * @returns 带描边的轻量面板
 */
export function ToolPanel({ children, className = "" }: ToolPanelProps) {
  return (
    <div className={`tool-panel ${className}`.trim()}>
      {children}
    </div>
  );
}
