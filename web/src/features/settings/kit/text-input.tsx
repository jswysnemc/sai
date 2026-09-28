import type { InputHTMLAttributes, TextareaHTMLAttributes, Ref } from "react";
import { cx } from "./class-names";
import { useFieldContext } from "./field-context";
import "./inputs.css";

type SkTextInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "onChange" | "size"> & {
  value: string;
  onChange: (value: string) => void;
  /** 使用等宽字体，适合地址、标识与命令 */
  mono?: boolean;
};

/**
 * 渲染设置页单行文本输入，自动关联所在字段的标签与说明。
 *
 * @param props 当前值、更新回调、是否等宽与原生输入属性
 * @returns 文本输入框
 */
export function SkTextInput({ value, onChange, mono, className, ...rest }: SkTextInputProps) {
  const field = useFieldContext();
  return (
    <input
      type="text"
      id={field?.controlId}
      aria-describedby={field?.hintId}
      aria-invalid={field?.invalid || undefined}
      spellCheck={false}
      autoComplete="off"
      {...rest}
      className={cx("sk-input", mono && "is-mono", className)}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}

type SkTextAreaProps = Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "value" | "onChange"> & {
  ref?: Ref<HTMLTextAreaElement>;
  value: string;
  onChange: (value: string) => void;
  mono?: boolean;
};

/**
 * 渲染设置页多行文本输入，自动关联所在字段的标签与说明。
 *
 * @param props 当前值、更新回调、是否等宽与原生多行输入属性
 * @returns 多行文本输入框
 */
export function SkTextArea({ value, onChange, mono, className, rows = 4, ...rest }: SkTextAreaProps) {
  const field = useFieldContext();
  return (
    <textarea
      id={field?.controlId}
      aria-describedby={field?.hintId}
      aria-invalid={field?.invalid || undefined}
      spellCheck={false}
      rows={rows}
      {...rest}
      className={cx("sk-textarea", mono && "is-mono", className)}
      value={value}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}
