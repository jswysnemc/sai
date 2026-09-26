import { createElement, forwardRef, type ForwardRefExoticComponent, type ReactNode, type RefAttributes, type SVGProps } from "react";
import { iconData, type IconName } from "./icon-data";

export type IconProps = SVGProps<SVGSVGElement> & {
  /** 显示边长，图形仍画在 24 视口内 */
  size?: number | string;
  strokeWidth?: number | string;
  absoluteStrokeWidth?: boolean;
  color?: string;
};

/** 与原先图标组件相同的可转发引用签名，供目录和菜单保存图标组件。 */
export type LucideIcon = ForwardRefExoticComponent<IconProps & RefAttributes<SVGSVGElement>>;

/**
 * 按名称创建已经对齐光学中心的图标。
 * @param name 图标名称
 * @returns 接受 size、描边和其余 SVG 属性的图标组件
 */
export function createIcon(name: IconName): LucideIcon {
  const datum = iconData[name];
  const [tx, ty, scale] = datum.fit;
  const Icon = forwardRef<SVGSVGElement, IconProps>(function Icon(
    { size = 24, strokeWidth = 2, absoluteStrokeWidth, color = "currentColor", children, ...rest },
    ref,
  ) {
    const numericSize = Number(size);
    const numericStroke = Number(strokeWidth);
    // 1. 绝对描边按 24 视口换算，避免图标缩小时线条变细
    const sw = absoluteStrokeWidth && Number.isFinite(numericSize) && numericSize > 0 && Number.isFinite(numericStroke)
      ? (numericStroke * 24) / numericSize
      : strokeWidth;
    return (
      <svg
        ref={ref}
        xmlns="http://www.w3.org/2000/svg"
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke={color}
        strokeWidth={sw}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
        {...rest}
      >
        <g transform={`translate(${tx} ${ty}) scale(${scale})`} vectorEffect="non-scaling-stroke">
          {datum.nodes.map(([tag, attrs], index) => createElement(tag, { ...attrs, key: index }))}
        </g>
        {children as ReactNode}
      </svg>
    );
  });
  Icon.displayName = name;
  return Icon;
}
