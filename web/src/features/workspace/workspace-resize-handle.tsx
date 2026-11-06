import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { useI18n } from "../i18n/use-i18n";
import { clampWorkspaceWidthForWorkbench, DEFAULT_WORKSPACE_WIDTH } from "./workspace-layout-state";

type WorkspaceResizeHandleProps = {
  width: number;
  swapped?: boolean;
  onResize: (width: number, workbenchWidth: number) => void;
};

/**
 * 渲染右侧工作区拖动手柄，并把指针位置转换为面板宽度。
 *
 * @param props 当前面板宽度、布局是否左右调换与调整宽度回调
 * @returns 工作区拖动手柄
 */
export function WorkspaceResizeHandle({ width, swapped = false, onResize }: WorkspaceResizeHandleProps) {
  const { t } = useI18n();
  const handleRef = useRef<HTMLDivElement>(null);
  const cleanupRef = useRef<(() => void) | null>(null);
  const [workbenchWidth, setWorkbenchWidth] = useState(0);

  useEffect(() => {
    const parent = handleRef.current?.parentElement;
    if (!parent) return;
    setWorkbenchWidth(parent.clientWidth);
    const observer = new ResizeObserver(([entry]) => setWorkbenchWidth(entry.contentRect.width));
    observer.observe(parent);
    return () => {
      observer.disconnect();
      cleanupRef.current?.();
    };
  }, []);

  /**
   * 开始监听全局指针移动，直到用户释放指针。
   *
   * @param event 手柄指针按下事件
   * @returns 无返回值
   */
  const handlePointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    event.preventDefault();
    cleanupRef.current?.();
    document.body.classList.add("workspace-resizing");
    const workbench = event.currentTarget.parentElement?.getBoundingClientRect();

    // 1. 【Web 工作台】【面板拖动】按布局方向计算指针到工作区边缘的距离
    /** 根据指针横坐标更新宽度；moveEvent 为指针移动事件，无返回值 */
    const handlePointerMove = (moveEvent: PointerEvent) => {
      const right = workbench?.right ?? window.innerWidth;
      const left = workbench?.left ?? 0;
      onResize(swapped ? moveEvent.clientX - left : right - moveEvent.clientX, right - left);
    };

    // 2. 【Web 工作台】【面板拖动】释放、取消或卸载时统一清理监听与拖动态
    /** 清理本次拖动的全局监听，无参数和返回值 */
    const handlePointerUp = () => {
      document.body.classList.remove("workspace-resizing");
      window.removeEventListener("pointermove", handlePointerMove);
      window.removeEventListener("pointerup", handlePointerUp);
      window.removeEventListener("pointercancel", handlePointerUp);
      cleanupRef.current = null;
    };

    window.addEventListener("pointermove", handlePointerMove);
    window.addEventListener("pointerup", handlePointerUp, { once: true });
    window.addEventListener("pointercancel", handlePointerUp, { once: true });
    cleanupRef.current = handlePointerUp;
  };

  /**
   * 【Web 工作台】【面板键盘】使用方向键调整宽度，Home 与 End 设置最小或最大宽度
   * @param event 分隔条键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    const availableWidth = event.currentTarget.parentElement?.clientWidth ?? workbenchWidth;
    const direction = swapped ? -1 : 1;
    const nextWidths: Partial<Record<string, number>> = {
      ArrowLeft: width + direction * 16,
      ArrowRight: width - direction * 16,
      Home: clampWorkspaceWidthForWorkbench(0, availableWidth),
      End: clampWorkspaceWidthForWorkbench(Infinity, availableWidth)
    };
    const nextWidth = nextWidths[event.key];
    if (nextWidth === undefined) return;
    event.preventDefault();
    onResize(nextWidth, availableWidth);
  };

  /** 双击恢复默认面板宽度，无参数和返回值 */
  const resetWidth = (): void => {
    onResize(DEFAULT_WORKSPACE_WIDTH, handleRef.current?.parentElement?.clientWidth ?? workbenchWidth);
  };

  return (
    <div
      ref={handleRef}
      className="workspace-resize-handle max-md:hidden"
      role="separator"
      tabIndex={0}
      aria-label={t("Resize workspace width", "调整工作区宽度")}
      aria-orientation="vertical"
      aria-valuemin={clampWorkspaceWidthForWorkbench(0, workbenchWidth)}
      aria-valuemax={Math.round(Math.max(width, clampWorkspaceWidthForWorkbench(Infinity, workbenchWidth)))}
      aria-valuenow={Math.round(width)}
      onKeyDown={handleKeyDown}
      onDoubleClick={resetWidth}
      onPointerDown={handlePointerDown}
    >
      <span />
    </div>
  );
}
