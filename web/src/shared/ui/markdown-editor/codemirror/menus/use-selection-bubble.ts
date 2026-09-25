import type { EditorView } from "@codemirror/view";
import { useCallback, useEffect, useRef, useState } from "react";

/** 气泡的锚点：视口坐标与朝向。 */
export type BubblePosition = {
  x: number;
  y: number;
  placement: "above" | "below";
};

/** 气泡高度的估算值（像素），用于判断上方是否放得下。 */
const BUBBLE_HEIGHT = 34;

/** 气泡与选区之间的间距（像素）。 */
const GAP = 6;

/**
 * 计算选区气泡的位置。
 *
 * @param view 编辑器视图
 * @returns 气泡位置；选区为空、编辑器失焦或选区滚出视口时为 null
 */
function measure(view: EditorView): BubblePosition | null {
  const selection = view.state.selection.main;
  if (selection.empty || !view.hasFocus) return null;
  const start = view.coordsAtPos(selection.from, 1);
  const end = view.coordsAtPos(selection.to, -1);
  if (!start || !end) return null;
  const bounds = view.scrollDOM.getBoundingClientRect();
  if (end.bottom < bounds.top || start.top > bounds.bottom) return null;
  // 单行选区居中于选区，跨行选区对齐选区起点
  const x = Math.abs(start.top - end.top) < 4 ? (start.left + end.right) / 2 : start.left;
  if (start.top - BUBBLE_HEIGHT - GAP >= bounds.top) return { x, y: start.top - GAP, placement: "above" };
  return { x, y: Math.min(end.bottom, bounds.bottom) + GAP, placement: "below" };
}

/**
 * 跟踪选区并给出格式气泡的位置。
 *
 * 拖选过程中不显示，松开鼠标或用 Shift+方向键调整选区后才出现，
 * 避免气泡跟着指针闪烁；滚动时逐帧重新定位。
 *
 * @param view 编辑器视图，创建前为 null
 * @param enabled 是否启用（仅预览模式且可编辑时）
 * @returns 气泡位置与供编辑器更新监听调用的刷新函数
 */
export function useSelectionBubble(view: EditorView | null, enabled: boolean) {
  const [position, setPosition] = useState<BubblePosition | null>(null);
  const dragging = useRef(false);
  const frame = useRef(0);

  const refresh = useCallback(() => {
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => {
      setPosition(enabled && view && !dragging.current ? measure(view) : null);
    });
  }, [enabled, view]);

  useEffect(() => {
    if (!view || !enabled) {
      setPosition(null);
      return;
    }
    // 1. 拖选期间隐藏，松开后再定位
    const onPointerDown = (event: PointerEvent) => {
      if (event.button !== 0) return;
      dragging.current = true;
      setPosition(null);
    };
    const onPointerUp = () => {
      if (!dragging.current) return;
      dragging.current = false;
      refresh();
    };
    // 2. 失焦隐藏，滚动与窗口尺寸变化时重新定位
    const onBlur = () => setPosition(null);
    view.contentDOM.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("pointerup", onPointerUp);
    view.contentDOM.addEventListener("blur", onBlur);
    view.scrollDOM.addEventListener("scroll", refresh, { passive: true });
    window.addEventListener("resize", refresh);
    return () => {
      cancelAnimationFrame(frame.current);
      view.contentDOM.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("pointerup", onPointerUp);
      view.contentDOM.removeEventListener("blur", onBlur);
      view.scrollDOM.removeEventListener("scroll", refresh);
      window.removeEventListener("resize", refresh);
    };
  }, [enabled, view, refresh]);

  return { position, refresh, hide: () => setPosition(null) };
}
