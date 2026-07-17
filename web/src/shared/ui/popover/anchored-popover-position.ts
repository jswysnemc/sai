export type AnchorRect = {
  left: number;
  right: number;
  top: number;
  bottom: number;
};

export type AnchoredPopoverOptions = {
  viewportWidth: number;
  viewportHeight: number;
  preferredWidth: number;
  minimumWidth: number;
  align: "left" | "right";
  /** 弹层内容期望的最大高度,与可用空间取较小值 */
  maxHeight?: number;
  padding?: number;
  gap?: number;
};

export type AnchoredPopoverPosition = {
  left: number;
  width: number;
  maxHeight: number;
  top?: number;
  bottom?: number;
};

/** 弹层的默认期望高度。 */
const DEFAULT_MAX_HEIGHT = 420;
/** 向下弹出所需的最小空间,低于该值时翻转到触发器上方。 */
const FLIP_THRESHOLD = 180;
/** 弹层高度下限,避免在极小空间里被压扁。 */
const MIN_HEIGHT = 120;

/**
 * 计算固定定位弹层的视口内坐标。
 *
 * 默认在触发器下方展开;下方空间不足且上方更宽裕时翻转到上方,
 * 并返回受可用空间约束的最大高度,避免弹层溢出视口被裁剪。
 *
 * @param anchor 触发器的视口坐标
 * @param options 视口尺寸、菜单尺寸和对齐方式
 * @returns 弹层坐标、宽度与最大高度
 */
export function calculateAnchoredPopoverPosition(
  anchor: AnchorRect,
  options: AnchoredPopoverOptions
): AnchoredPopoverPosition {
  const padding = options.padding ?? 12;
  const gap = options.gap ?? 6;
  const availableWidth = Math.max(0, options.viewportWidth - padding * 2);
  const minimumWidth = Math.min(Math.max(options.minimumWidth, 0), availableWidth);
  const width = Math.min(Math.max(options.preferredWidth, minimumWidth), availableWidth);
  const preferredLeft = options.align === "right" ? anchor.right - width : anchor.left;
  const left = Math.max(padding, Math.min(preferredLeft, options.viewportWidth - width - padding));
  const desired = options.maxHeight ?? DEFAULT_MAX_HEIGHT;
  const spaceBelow = options.viewportHeight - anchor.bottom - gap - padding;
  const spaceAbove = anchor.top - gap - padding;
  // 1. 下方空间足够或不逊于上方时向下弹出,否则翻转到触发器上方
  if (spaceBelow >= Math.min(desired, FLIP_THRESHOLD) || spaceBelow >= spaceAbove) {
    return {
      left,
      width,
      top: anchor.bottom + gap,
      maxHeight: clampHeight(desired, spaceBelow)
    };
  }
  return {
    left,
    width,
    bottom: options.viewportHeight - anchor.top + gap,
    maxHeight: clampHeight(desired, spaceAbove)
  };
}

/** 把期望高度夹取到可用空间内,并保留可用下限。 */
function clampHeight(desired: number, space: number): number {
  return Math.min(desired, Math.max(space, MIN_HEIGHT));
}
