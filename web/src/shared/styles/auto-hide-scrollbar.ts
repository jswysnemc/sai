/**
 * 滚动时短暂显示滚动条。
 *
 * 在捕获阶段监听 document 滚动，给实际滚动容器加上 `is-scrolling`，
 * 空闲一段时间后移除。配合 `scrollbar.css` 实现默认隐藏、滚动显示。
 */

const SCROLLBAR_IDLE_MS = 1000;
const SCROLLING_CLASS = "is-scrolling";
const hideTimers = new WeakMap<Element, number>();

/**
 * 标记元素处于滚动态，并在空闲后移除标记。
 *
 * @param element 实际发生滚动的元素
 * @returns 无返回值
 */
function markScrolling(element: Element): void {
  element.classList.add(SCROLLING_CLASS);
  const previous = hideTimers.get(element);
  if (previous !== undefined) {
    window.clearTimeout(previous);
  }
  const timer = window.setTimeout(() => {
    element.classList.remove(SCROLLING_CLASS);
    hideTimers.delete(element);
  }, SCROLLBAR_IDLE_MS);
  hideTimers.set(element, timer);
}

/**
 * 从滚动事件目标解析应显示滚动条的元素。
 *
 * @param target 事件目标
 * @returns 可滚动元素；无法解析时返回 null
 */
function resolveScrollElement(target: EventTarget | null): Element | null {
  if (target instanceof Element) {
    return target;
  }
  if (target === document || target === document.documentElement) {
    return document.documentElement;
  }
  return null;
}

/**
 * 启用全局滚动条自动隐藏行为。
 *
 * @returns 取消监听的清理函数
 */
export function enableAutoHideScrollbars(): () => void {
  /**
   * 处理任意容器的滚动事件。
   *
   * @param event 滚动事件
   * @returns 无返回值
   */
  const onScroll = (event: Event) => {
    const element = resolveScrollElement(event.target);
    if (element) markScrolling(element);
  };

  document.addEventListener("scroll", onScroll, { capture: true, passive: true });
  return () => {
    document.removeEventListener("scroll", onScroll, true);
  };
}
