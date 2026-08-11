import type { ReactNode } from "react";

type ToolPanelProps = {
  /** 面板内容 */
  children: ReactNode;
  /** 附加类名 */
  className?: string;
};

/**
 * 工具卡片展开区的内容面板。
 *
 * 展开区常同时容纳命令、输出、错误多段等宽文本，彼此都是灰底灰字；
 * 没有边界时几段会连成一片，读者需要逐行辨认哪段属于哪部分。
 * 统一包一层描边面板后，"这一块是一个整体"由容器表达，
 * 段落之间不必再靠额外分隔线区分。
 *
 * @param props 面板内容与附加类名
 * @returns 带描边与内边距的面板
 */
export function ToolPanel({ children, className = "" }: ToolPanelProps) {
  return (
    <div className={`min-w-0 rounded-xl border border-border bg-panel px-4 py-3 ${className}`.trim()}>
      {children}
    </div>
  );
}
