import type { ReactNode } from "react";

type InlineDiffPreviewProps = {
  /** diff 内容 */
  children: ReactNode;
};

/**
 * 工具卡片内嵌 diff 预览的容器。
 *
 * 给 diff 一个明确的边框与限高：卡片展开区里往往还有命令、输出等其它内容，
 * 没有边界时长 diff 会与上下文糊成一片，读不出"这一段是文件改动"。
 * 超出限高的部分在容器内滚动，不把整张卡片撑到需要翻页。
 *
 * @param props 内嵌的 diff 内容
 * @returns 带边框与限高的预览容器
 */
export function InlineDiffPreview({ children }: InlineDiffPreviewProps) {
  return (
    <div
      className="mb-2 max-h-60 overflow-auto rounded-xl border border-border bg-card"
      data-inline-diff-preview
    >
      {children}
    </div>
  );
}
