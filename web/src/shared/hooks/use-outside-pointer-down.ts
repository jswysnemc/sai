import { useEffect, useRef, type RefObject } from "react";

/**
 * 在指定根元素外按下指针时执行回调。
 *
 * @param rootRef 需要保留交互的根元素引用
 * @param onOutside 外部按下指针时执行的方法
 * @param enabled 是否启用外部点击监听
 * @returns 无返回值
 */
export function useOutsidePointerDown(
  rootRef: RefObject<HTMLElement | null>,
  onOutside: () => void,
  enabled = true
): void {
  const callbackRef = useRef(onOutside);
  callbackRef.current = onOutside;

  useEffect(() => {
    if (!enabled) return;

    // 1. 根元素不包含事件目标时执行关闭逻辑
    const handlePointerDown = (event: PointerEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) callbackRef.current();
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [enabled, rootRef]);
}
