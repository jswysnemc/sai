import { isNearOutputBottom } from "./use-follow-output-scroll";

export type HistoryScrollAnchor = {
  container: HTMLElement;
  anchor: Element;
  offset: number;
  scrollTop: number;
  atBottom: boolean;
};

/**
 * 【会话载入】【滚动锚点】记录视口内首个完整区块的位置，使上方历史展开后保持阅读位置。
 * @param section 即将挂载正文的历史区块
 * @returns 滚动容器、共同锚点与原位置；没有消息滚动容器时为空
 */
export function captureHistoryScroll(section: HTMLElement): HistoryScrollAnchor | null {
  const container = section.closest<HTMLElement>(".message-scroll");
  if (!container) return null;
  const viewportTop = container.getBoundingClientRect().top;
  let anchor: Element = section;
  while (anchor.getBoundingClientRect().top < viewportTop && anchor.nextElementSibling) {
    anchor = anchor.nextElementSibling;
  }
  return {
    container,
    anchor,
    offset: anchor.getBoundingClientRect().top - viewportTop,
    scrollTop: container.scrollTop,
    atBottom: isNearOutputBottom(container),
  };
}

/**
 * 【会话载入】【位置保持】补偿历史正文展开造成的位移，用户已经滚动时保留用户位置。
 * @param snapshot 挂载前捕获的共同锚点
 * @returns 无返回值
 */
export function restoreHistoryScroll(snapshot: HistoryScrollAnchor | null): void {
  if (!snapshot) return;
  const { container, anchor, scrollTop, offset, atBottom } = snapshot;
  if (!anchor.isConnected || !container.contains(anchor) || Math.abs(container.scrollTop - scrollTop) > 1) return;
  if (atBottom) {
    container.scrollTop = container.scrollHeight;
    return;
  }
  container.scrollTop += anchor.getBoundingClientRect().top - container.getBoundingClientRect().top - offset;
}
