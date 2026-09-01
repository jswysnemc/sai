import type { ReactNode } from "react";

type MemoryToolsRowProps = {
  children: ReactNode;
};

/**
 * 记忆页底部工具区：注入索引预览与逐出上下文检索并排放置。
 *
 * 两者都是低频诊断入口，并排而非纵排可以少占一整段纵向空间，
 * 条目列表因此能占据首屏主要高度。
 *
 * @param props 工具子节点
 * @returns 工具区容器
 */
export function MemoryToolsRow({ children }: MemoryToolsRowProps) {
  return <div className="memory-tools-row">{children}</div>;
}
