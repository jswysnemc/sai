import type { CSSProperties, ReactNode } from "react";
import "./hover-reveal-button.css";

type HoverRevealButtonProps = {
  /** 常驻显示的图标 */
  icon: ReactNode;
  /** 悬停或聚焦时展开的说明文字 */
  label: string;
  className?: string;
  /** 内联定位样式，供浮动场景传入 */
  style?: CSSProperties;
  onClick: () => void;
};

/**
 * 悬停展开为「图标 + 文字」的圆角浮动按钮。
 *
 * 静止时只占一个圆形图标，不干扰正文；指针悬停或键盘聚焦时向一侧展开文字说明，
 * 让首次使用的人知道按钮做什么。文字宽度用 grid 的 0fr → 1fr 过渡，
 * 因此不需要为每处按钮硬编码展开宽度。
 *
 * @param props 图标、说明文字、附加类名与点击回调
 * @returns 浮动按钮
 */
export function HoverRevealButton({ icon, label, className, style, onClick }: HoverRevealButtonProps) {
  return (
    <button
      type="button"
      className={`hover-reveal-button${className ? ` ${className}` : ""}`}
      style={style}
      onClick={onClick}
      aria-label={label}
      title={label}
    >
      <span className="hover-reveal-button-icon" aria-hidden>{icon}</span>
      <span className="hover-reveal-button-text" aria-hidden>
        <span>{label}</span>
      </span>
    </button>
  );
}
