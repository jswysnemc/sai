import type { PointerEvent as ReactPointerEvent } from "react";
import { useI18n } from "../i18n/use-i18n";

type WorkspaceResizeHandleProps = {
  swapped?: boolean;
  onResize: (width: number, workbenchWidth: number) => void;
};

/**
 * 渲染右侧工作区拖动手柄，并把指针位置转换为面板宽度。
 *
 * @param props 布局是否左右调换与调整宽度回调
 * @returns 工作区拖动手柄
 */
export function WorkspaceResizeHandle({ swapped = false, onResize }: WorkspaceResizeHandleProps) {
  const { t } = useI18n();
  /**
   * 开始监听全局指针移动，直到用户释放指针。
   *
   * @param event 手柄指针按下事件
   */
  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    document.body.classList.add("workspace-resizing");
    const workbench = event.currentTarget.parentElement?.getBoundingClientRect();

    // 1. 常规布局工作区在右侧，按指针到主区右缘的距离计算宽度；调换后按到左缘的距离
    const handlePointerMove = (moveEvent: PointerEvent) => {
      const right = workbench?.right ?? window.innerWidth;
      const left = workbench?.left ?? 0;
      onResize(swapped ? moveEvent.clientX - left : right - moveEvent.clientX, right - left);
    };

    // 2. 释放指针后统一清理全局监听器
    const handlePointerUp = () => {
      document.body.classList.remove("workspace-resizing");
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
  };

  return (
    <div
      className="workspace-resize-handle"
      role="separator"
      aria-label={t("Resize workspace width", "调整工作区宽度")}
      aria-orientation="vertical"
      onPointerDown={handlePointerDown}
    >
      <span />
    </div>
  );
}
