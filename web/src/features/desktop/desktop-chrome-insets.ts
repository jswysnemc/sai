/** 桌面窗口按钮占用的顶行区域（视口 CSS 像素）。 */
export type DesktopReserve = {
  /** 右上角让出的宽度，Windows/Linux 为窗口按钮 */
  right: number;
  /** 左上角让出的宽度，macOS 为红绿灯 */
  left: number;
  /** 顶行高度 */
  height: number;
};

/** 顶行元素的视口位置。 */
export type RowRect = { left: number; right: number; top: number; bottom: number };

/** 某个顶行元素需要额外留出的左右内边距（视口像素）。 */
export type RowInset = { left: number; right: number };

/**
 * 计算顶行元素与窗口按钮重叠的宽度。
 *
 * 只有贴着窗口顶端、且横向覆盖到按钮区域的元素才需要让位；
 * 让出的宽度等于重叠部分，元素本身离窗口边缘越远让得越少。
 *
 * @param rect 元素视口位置
 * @param viewportWidth 视口宽度
 * @param reserve 窗口按钮占用区域
 * @returns 左右需要额外留出的宽度
 */
export function rowInset(rect: RowRect, viewportWidth: number, reserve: DesktopReserve): RowInset {
  const touchesTop = rect.top < reserve.height && rect.bottom > 0;
  if (!touchesTop || rect.right <= rect.left) return { left: 0, right: 0 };
  const rightStart = viewportWidth - reserve.right;
  const right = reserve.right > 0 ? Math.max(0, rect.right - Math.max(rect.left, rightStart)) : 0;
  const left = reserve.left > 0 ? Math.max(0, Math.min(rect.right, reserve.left) - rect.left) : 0;
  return { left: round(left), right: round(right) };
}

/**
 * 保留两位小数，避免亚像素抖动反复写入样式。
 *
 * @param value 原始宽度
 * @returns 取整后的宽度
 */
function round(value: number): number {
  return Math.round(value * 100) / 100;
}
