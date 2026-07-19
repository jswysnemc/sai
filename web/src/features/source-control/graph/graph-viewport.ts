export type GraphViewport = {
  scrollTop: number;
  height: number;
};

type GraphViewportElement = Pick<HTMLElement, "scrollTop" | "clientHeight">;

/**
 * 立即复制提交图滚动容器的视口数据，避免异步状态更新读取已释放的事件目标。
 *
 * @param element 当前滚动容器
 * @returns 与 DOM 生命周期无关的视口快照
 */
export function snapshotGraphViewport(element: GraphViewportElement): GraphViewport {
  return {
    scrollTop: element.scrollTop,
    height: element.clientHeight
  };
}
