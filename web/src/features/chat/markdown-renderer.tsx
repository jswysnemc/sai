import { memo, useDeferredValue, type ReactNode } from "react";
import type { MarkdownStylePreferences } from "../markdown/markdown-style-preferences";
import { useMarkdownStylePreferences } from "../markdown/markdown-style-store";
import { EMPTY_INLINE_ATOMS, MarkdownContent } from "./markdown-content";

/**
 * 【前端性能】【流式调度】延后流式文本更新，并将昂贵解析隔离在稳定的子组件中。
 * @param props Markdown 源文、内联原子、外观偏好及流式标记
 * @returns 支持代码、公式与图表的 Markdown 内容
 */
export const MarkdownRenderer = memo(function MarkdownRenderer({
  source,
  inlineAtoms = EMPTY_INLINE_ATOMS,
  stylePreferences,
  streaming = false,
  collapseJson = false
}: {
  source: string;
  inlineAtoms?: readonly ReactNode[];
  stylePreferences?: MarkdownStylePreferences;
  streaming?: boolean;
  /** 上下文工具区默认折叠参数定义，不影响普通聊天代码块 */
  collapseJson?: boolean;
}) {
  const storedStyle = useMarkdownStylePreferences();
  const deferredSource = useDeferredValue(source);
  return (
    <MarkdownContent
      source={streaming ? deferredSource : source}
      inlineAtoms={inlineAtoms}
      style={stylePreferences ?? storedStyle.preferences}
      streaming={streaming}
      collapseJson={collapseJson}
    />
  );
});
