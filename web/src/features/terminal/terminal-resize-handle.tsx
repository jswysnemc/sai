import type { PointerEvent as ReactPointerEvent } from "react";

/**
 * 渲染底部终端高度调整手柄。
 *
 * @param props 高度更新回调
 * @returns 水平调整手柄
 */
export function TerminalResizeHandle({ onResize }: { onResize: (height: number) => void }) {
  /** 监听指针移动并换算终端高度。 */
  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    document.body.classList.add("terminal-resizing");
    const handlePointerMove = (moveEvent: PointerEvent) => onResize(window.innerHeight - moveEvent.clientY);
    const handlePointerUp = () => {
      document.body.classList.remove("terminal-resizing");
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };
    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
  };
  return <div className="terminal-resize-handle" role="separator" aria-label="调整终端高度" aria-orientation="horizontal" onPointerDown={handlePointerDown}><span /></div>;
}
