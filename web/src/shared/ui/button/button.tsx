import type { ButtonHTMLAttributes, ReactNode } from "react";
import "./button.css";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  /**
   * 视觉变体。
   *
   * danger 是实心警示色，用于对话框确认这类需要明确阻断的主操作；
   * ghost-danger 是透明底红字，用于列表与工具栏里密集排布的破坏性动作。
   */
  variant?: "primary" | "secondary" | "danger" | "ghost-danger";
  children: ReactNode;
};

/**
 * 渲染项目统一按钮，并保留原生按钮可访问属性。
 *
 * @param props 按钮类型、内容和原生按钮属性
 * @returns 统一样式按钮
 */
export function Button({ variant = "secondary", className = "", children, ...props }: ButtonProps) {
  return <button type="button" className={`ui-button ${variant}${className ? ` ${className}` : ""}`} {...props}>{children}</button>;
}
