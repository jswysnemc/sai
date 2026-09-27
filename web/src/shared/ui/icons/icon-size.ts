/**
 * 图标尺寸阶梯。
 * - 12：角标、紧凑树节点折叠
 * - 14：列表辅助图标、行内图文混排
 * - 16：标准按钮与表单控件
 * - 20：顶栏核心操作
 */
export const ICON_SIZES = [12, 14, 16, 20] as const;
export type IconSize = (typeof ICON_SIZES)[number];

/** 各级尺寸的光学描边宽度（CSS 像素），小尺寸收窄以免线条糊成一团。 */
export const OPTICAL_STROKE_PX: Record<IconSize, number> = { 12: 1.25, 14: 1.25, 16: 1.5, 20: 1.75 };

/**
 * 换算写在 24 视口上的描边宽度。
 *
 * Lucide 图形统一画在 24 视口内，显示尺寸缩小时描边随之变细；
 * 按像素宽度换算后，不同尺寸的图标在屏幕上保持稳定的线宽。
 * @param size 显示边长
 * @param strokeWidth 调用方显式描边，缺省走光学阶梯
 * @param absolute 显式描边是否为 CSS 像素
 * @returns 24 视口单位下的描边宽度
 */
export function resolveStrokeWidth(size: IconSize, strokeWidth?: number, absolute?: boolean): number {
  const unitsPerPixel = 24 / size;
  // 1. 缺省描边：按阶梯的像素宽度换算
  if (strokeWidth === undefined) return OPTICAL_STROKE_PX[size] * unitsPerPixel;
  // 2. 显式像素描边：同样换算
  if (absolute) return strokeWidth * unitsPerPixel;
  // 3. 显式视口描边：原样使用
  return strokeWidth;
}
