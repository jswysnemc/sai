import { useMemo } from "react";
import { hasAnsi, parseAnsi } from "./ansi-parse";

/**
 * 渲染可能含 ANSI 着色的命令输出文本。
 *
 * 不含转义序列时直接返回原文，不产生任何额外节点——
 * 绝大多数输出走这条路径，包一层 span 只会拖慢长输出的渲染。
 *
 * @param props source 为原始输出文本
 * @returns 着色分段或原始文本
 */
export function AnsiOutput({ source }: { source: string }) {
  // 分段是纯计算，父组件因计时器重渲染时不应重跑
  const segments = useMemo(() => (hasAnsi(source) ? parseAnsi(source) : null), [source]);
  if (!segments) return <>{source}</>;
  return (
    <>
      {segments.map((segment, index) => (
        <span
          className={segmentClassName(segment.color, segment.bold, segment.dim)}
          key={index}
        >
          {segment.text}
        </span>
      ))}
    </>
  );
}

/**
 * 组装分段的样式类名。
 *
 * @param color 前景色语义名
 * @param bold 是否加粗
 * @param dim 是否变暗
 * @returns 类名字符串
 */
function segmentClassName(color: string, bold: boolean, dim: boolean): string {
  return [color ? `ansi-fg-${color}` : "", bold ? "ansi-bold" : "", dim ? "ansi-dim" : ""]
    .filter(Boolean)
    .join(" ");
}
