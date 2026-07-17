import type { ButtonHTMLAttributes, ReactNode } from "react";
import "./button.css";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "danger";
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
