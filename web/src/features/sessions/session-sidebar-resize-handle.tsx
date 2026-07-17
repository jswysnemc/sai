import type { KeyboardEvent, PointerEvent as ReactPointerEvent } from "react";
import { SESSION_SIDEBAR_MAX_WIDTH, SESSION_SIDEBAR_MIN_WIDTH } from "./use-session-sidebar-layout";
import { useI18n } from "../i18n/use-i18n";

type SessionSidebarResizeHandleProps = {
  width: number;
  onResize: (width: number) => void;
};

/**
 * 渲染会话侧栏拖动手柄，并支持键盘微调宽度。
 *
 * @param props 当前宽度和调整宽度回调
 * @returns 会话侧栏拖动手柄
 */
export function SessionSidebarResizeHandle({ width, onResize }: SessionSidebarResizeHandleProps) {
  const { t } = useI18n();
  /**
   * 开始监听全局指针移动，直到用户释放指针。
   *
   * @param event 手柄指针按下事件
   */
  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const sidebar = event.currentTarget.parentElement?.getBoundingClientRect();
    document.body.classList.add("session-sidebar-resizing");

    // 1. 根据指针到侧栏左边缘的距离计算新宽度
    const handlePointerMove = (moveEvent: PointerEvent) => {
      onResize(moveEvent.clientX - (sidebar?.left ?? 0));
    };

    // 2. 释放指针后清理全部全局监听器
    const handlePointerUp = () => {
      document.body.classList.remove("session-sidebar-resizing");
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
    window.addEventListener("pointercancel", handlePointerUp, { once: true });
  };

  /**
   * 使用方向键、Home 和 End 调整侧栏宽度。
   *
   * @param event 手柄键盘事件
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const widths: Partial<Record<string, number>> = {
      ArrowLeft: width - 12,
      ArrowRight: width + 12,
      Home: SESSION_SIDEBAR_MIN_WIDTH,
      End: SESSION_SIDEBAR_MAX_WIDTH
    };
    const nextWidth = widths[event.key];
    if (nextWidth === undefined) return;
    event.preventDefault();
    onResize(nextWidth);
  };

  return (
    <div
      className="session-sidebar-resize-handle"
      role="separator"
      tabIndex={0}
      aria-label={t("Resize session sidebar width", "调整会话侧栏宽度")}
      aria-orientation="vertical"
      aria-valuemin={SESSION_SIDEBAR_MIN_WIDTH}
      aria-valuemax={SESSION_SIDEBAR_MAX_WIDTH}
      aria-valuenow={Math.round(width)}
      onPointerDown={handlePointerDown}
      onKeyDown={handleKeyDown}
    >
      <span />
    </div>
  );
}
