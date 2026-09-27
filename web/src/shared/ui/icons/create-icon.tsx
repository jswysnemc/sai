import { createElement, forwardRef, type ForwardRefExoticComponent, type ReactNode, type RefAttributes, type SVGProps } from "react";
import { iconData, type IconName } from "./icon-data";

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
const OPTICAL_STROKE_PX: Record<IconSize, number> = { 12: 1.25, 14: 1.25, 16: 1.5, 20: 1.75 };

export type IconProps = Omit<SVGProps<SVGSVGElement>, "strokeWidth"> & {
  /** 显示边长，只能取尺寸阶梯中的一级；图形仍画在 24 视口内 */
  size?: IconSize;
  /** 显式描边：默认按 24 视口单位解释；缺省时按尺寸阶梯取光学描边 */
  strokeWidth?: number;
  /** 为 true 时 strokeWidth 表示渲染后的 CSS 像素宽度 */
  absoluteStrokeWidth?: boolean;
  color?: string;
};

/** 与原先图标组件相同的可转发引用签名，供目录和菜单保存图标组件。 */
export type LucideIcon = ForwardRefExoticComponent<IconProps & RefAttributes<SVGSVGElement>>;

/**
 * 计算写在光学分组上的描边宽度（分组坐标单位）。
 *
 * 分组经 `scale` 放大后描边也随之放大，因此统一除以缩放量，
 * 保证同一尺寸下 Chevron、箭头等放大过的图形与其他图标线宽一致。
 * @param size 显示边长
 * @param scale 光学分组缩放量
 * @param strokeWidth 调用方显式描边，缺省走光学阶梯
 * @param absolute 显式描边是否为 CSS 像素
 * @returns 分组内使用的描边宽度
 */
export function resolveStrokeWidth(size: IconSize, scale: number, strokeWidth?: number, absolute?: boolean): number {
  const unitsPerPixel = 24 / size;
  // 1. 缺省描边：按阶梯的像素宽度再换算到 24 视口
  if (strokeWidth === undefined) return (OPTICAL_STROKE_PX[size] * unitsPerPixel) / scale;
  // 2. 显式像素描边：换算到 24 视口
  if (absolute) return (strokeWidth * unitsPerPixel) / scale;
  // 3. 显式视口描边：只抵消分组缩放
  return strokeWidth / scale;
}

/**
 * 按名称创建已经对齐光学中心的图标。
 * @param name 图标名称
 * @returns 接受 size、描边和其余 SVG 属性的图标组件
 */
export function createIcon(name: IconName): LucideIcon {
  const datum = iconData[name];
  const [tx, ty, scale] = datum.fit;
  // 1. 沿用 lucide 的 kebab 规则，测试和样式仍按 lucide-folder-git2 这类类名识别
  const lucideClass = `lucide lucide-${name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase()}`;
  const Icon = forwardRef<SVGSVGElement, IconProps>(function Icon(
    { size = 16, strokeWidth, absoluteStrokeWidth, color = "currentColor", children, className, ...rest },
    ref,
  ) {
    // 2. 描边写在分组上并抵消分组缩放，外层 svg 不再设置描边宽度
    const groupStroke = resolveStrokeWidth(size, scale, strokeWidth, absoluteStrokeWidth);
    return (
      <svg
        ref={ref}
        xmlns="http://www.w3.org/2000/svg"
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke={color}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
        className={className ? `${lucideClass} ${className}` : lucideClass}
        {...rest}
      >
        <g transform={`translate(${tx} ${ty}) scale(${scale})`} strokeWidth={Number(groupStroke.toFixed(4))}>
          {datum.nodes.map(([tag, attrs], index) => createElement(tag, { ...attrs, key: index }))}
        </g>
        {children as ReactNode}
      </svg>
    );
  });
  Icon.displayName = name;
  return Icon;
}
