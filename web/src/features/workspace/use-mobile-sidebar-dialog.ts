import { useEffect, useRef, type RefObject } from "react";

/**
 * 将移动端侧栏作为模态抽屉处理，支持退出、焦点循环和焦点恢复。
 * @param open 抽屉当前是否处于移动端打开状态
 * @param containerRef 侧栏容器引用
 * @param onClose 关闭抽屉的回调
 * @returns 无返回值
 */
export function useMobileSidebarDialog(open: boolean, containerRef: RefObject<HTMLElement | null>, onClose: () => void): void {
  const closeRef = useRef(onClose);
  closeRef.current = onClose;
  useEffect(() => {
    if (!open) return;
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const focusFrame = requestAnimationFrame(() => containerRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus());

    /**
     * 处理抽屉本身的键盘事件，独立弹出的菜单与对话框自行处理。
     * @param event 当前键盘事件
     * @returns 无返回值
     */
    const handleKeyDown = (event: KeyboardEvent) => {
      const container = containerRef.current;
      if (!container || event.defaultPrevented || !(event.target instanceof Node) || !container.contains(event.target)) return;
      if (event.key === "Escape") {
        event.preventDefault();
        closeRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = Array.from(container.querySelectorAll<HTMLElement>('button:not(:disabled), a[href], input:not(:disabled), [tabindex]'))
        .filter((element) => element.tabIndex >= 0 && element.getClientRects().length > 0 && !element.closest("[inert]"));
      const first = focusable[0];
      const last = focusable.at(-1);
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    };
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      cancelAnimationFrame(focusFrame);
      document.removeEventListener("keydown", handleKeyDown);
      previousFocus?.focus();
    };
  }, [containerRef, open]);
}
