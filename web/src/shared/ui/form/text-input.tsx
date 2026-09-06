import type { InputHTMLAttributes, Ref } from "react";
import "./text-input.css";

/**
 * 渲染统一的单行输入框，供表单和对话框复用。
 * @param props 输入属性、引用和附加样式
 * @returns 单行输入组件
 */
export function TextInput({ className = "", ...props }: InputHTMLAttributes<HTMLInputElement> & { ref?: Ref<HTMLInputElement> }) {
  return <input className={`ui-text-input ${className}`.trim()} {...props} />;
}
