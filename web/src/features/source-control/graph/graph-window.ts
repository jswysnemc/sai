export type GitGraphWindow = {
  start: number;
  end: number;
  offsetTop: number;
  totalHeight: number;
};

/**
 * 计算固定行高提交图中需要挂载的可见区间。
 *
 * @param itemCount 提交总数
 * @param rowHeight 单行高度，单位为像素
 * @param scrollTop 容器垂直滚动距离
 * @param viewportHeight 容器可视高度
 * @param overscan 视口上下额外保留行数
 * @returns 左闭右开的提交索引区间及占位尺寸
 */
export function calculateGitGraphWindow(
  itemCount: number,
  rowHeight: number,
  scrollTop: number,
  viewportHeight: number,
  overscan = 6
): GitGraphWindow {
  const count = Math.max(0, Math.floor(itemCount));
  const height = Math.max(1, rowHeight);
  const safeScrollTop = Math.max(0, scrollTop);
  const safeViewportHeight = Math.max(height, viewportHeight);
  const safeOverscan = Math.max(0, Math.floor(overscan));
  const firstVisible = Math.min(Math.max(0, count - 1), Math.floor(safeScrollTop / height));
  const visibleCount = Math.ceil(safeViewportHeight / height);
  const start = Math.min(count, Math.max(0, firstVisible - safeOverscan));
  const end = Math.min(count, firstVisible + visibleCount + safeOverscan);
  return {
    start,
    end: Math.max(start, end),
    offsetTop: start * height,
    totalHeight: count * height
  };
}
