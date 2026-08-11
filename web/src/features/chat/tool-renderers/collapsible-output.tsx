import { useEffect, useMemo, useRef } from "react";
import { AnsiOutput } from "./ansi-output";
import "./collapsible-output.css";

/** 超过此高度时在输出块内滚动，避免撑满整段对话 */
const SCROLL_MAX_HEIGHT_PX = 260;

type CollapsibleOutputProps = {
  source: string;
  /**
   * 输出块的基础类名。
   *
   * 命令输出与通用工具输出各有一套内边距，两者都需要限高滚动，
   * 因此把版式类交给调用方决定，本组件只负责限高与着色。
   */
  className?: string;
};

/**
 * 渲染工具输出：完整展示内容，超长时在块内滚动，不再展开/收起。
 *
 * @param props source 为输出文本，className 为输出块类名
 * @returns 可滚动的输出块
 */
export function CollapsibleOutput({ source, className = "shell-output" }: CollapsibleOutputProps) {
  const preRef = useRef<HTMLPreElement | null>(null);
  const lineCount = useMemo(() => source.split("\n").length, [source]);


  return (
    <div className="collapsible-output">
      <pre ref={preRef} className={`${className} is-scrollable`}>
        <code>
          <AnsiOutput source={source} />
        </code>
      </pre>
    </div>
  );
}
