import { forwardRef, type TextareaHTMLAttributes } from "react";
import "./text-area.css";

type TextAreaProps = TextareaHTMLAttributes<HTMLTextAreaElement>;

/**
 * 渲染项目统一多行文本输入组件。
 *
 * @param props 原生多行输入属性
 * @returns 统一样式多行输入框
 */
export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>(function TextArea(
  { className = "", ...props },
  ref
) {
  return <textarea ref={ref} className={`ui-text-area${className ? ` ${className}` : ""}`} {...props} />;
});
