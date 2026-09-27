import { forwardRef, type ForwardRefExoticComponent, type RefAttributes, type SVGProps } from "react";
import type { LucideProps } from "lucide-react";
import { resolveStrokeWidth, type IconSize } from "./icon-size";

export type IconProps = Omit<SVGProps<SVGSVGElement>, "strokeWidth" | "ref"> & {
  /** 显示边长，只能取尺寸阶梯中的一级 */
  size?: IconSize;
  /** 显式描边：默认按 24 视口单位解释；缺省时按尺寸阶梯取光学描边 */
  strokeWidth?: number;
  /** 为 true 时 strokeWidth 表示渲染后的 CSS 像素宽度 */
  absoluteStrokeWidth?: boolean;
  color?: string;
};

/** 项目统一的图标组件签名，Lucide 图标与自绘图标共用。 */
export type LucideIcon = ForwardRefExoticComponent<IconProps & RefAttributes<SVGSVGElement>>;

/**
 * 为 Lucide 图标套上项目的尺寸阶梯与光学描边。
 *
 * 图形保持 Lucide 原始几何，不做缩放或平移；只统一尺寸取值、
 * 描边换算和默认的 aria-hidden，使 Lucide 与自绘图标视觉一致。
 * @param Icon lucide-react 图标组件或 createLucideIcon 生成的自绘图标
 * @returns 接受 size、描边和其余 SVG 属性的图标组件
 */
export function withIconDefaults(Icon: ForwardRefExoticComponent<LucideProps & RefAttributes<SVGSVGElement>>): LucideIcon {
  const Wrapped = forwardRef<SVGSVGElement, IconProps>(function SaiIcon(
    { size = 16, strokeWidth, absoluteStrokeWidth, ...rest },
    ref
  ) {
    // 描边统一换算到 24 视口，不再交给 lucide 按显示尺寸二次换算
    return (
      <Icon
        ref={ref}
        size={size}
        strokeWidth={Number(resolveStrokeWidth(size, strokeWidth, absoluteStrokeWidth).toFixed(4))}
        aria-hidden="true"
        {...(rest as LucideProps)}
      />
    );
  });
  Wrapped.displayName = Icon.displayName;
  return Wrapped;
}
