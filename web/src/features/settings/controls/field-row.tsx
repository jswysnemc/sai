import type { ReactNode } from "react";
import { Select, type SelectOption } from "../../../shared/ui/select/select";

type FieldRowBaseProps = {
  /** 主标签 */
  label: string;
  /** 弱化说明；缺省不渲染 */
  hint?: ReactNode;
};

/**
 * 渲染单行文本字段：标签 + 输入框 + 弱化说明。
 *
 * @param props 标签、说明、当前值与更新回调
 * @returns 文本字段行
 */
export function TextFieldRow({
  label,
  hint,
  value,
  placeholder,
  onChange
}: FieldRowBaseProps & {
  value: string;
  placeholder?: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="settings-field">
      <span>{label}</span>
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(event) => onChange(event.target.value)}
        spellCheck={false}
        autoComplete="off"
      />
      {hint != null && hint !== "" && <small>{hint}</small>}
    </label>
  );
}

/**
 * 渲染数值字段行；提供 min/max 时输入被截断到范围内。
 *
 * @param props 标签、说明、范围、当前值与更新回调
 * @returns 数值字段行
 */
export function NumberFieldRow({
  label,
  hint,
  value,
  min,
  max,
  onChange
}: FieldRowBaseProps & {
  value: number;
  min?: number;
  max?: number;
  onChange: (value: number) => void;
}) {
  /**
   * 将输入值收进允许范围。
   *
   * @param raw 原始输入数值
   * @returns 截断后的数值
   */
  const clamp = (raw: number): number => {
    let next = raw;
    if (typeof min === "number") next = Math.max(min, next);
    if (typeof max === "number") next = Math.min(max, next);
    return next;
  };
  return (
    <label className="settings-field">
      <span>{label}</span>
      <input
        type="number"
        min={min}
        max={max}
        value={value}
        onChange={(event) => onChange(clamp(Number(event.target.value)))}
      />
      {hint != null && hint !== "" && <small>{hint}</small>}
    </label>
  );
}

/**
 * 渲染枚举下拉字段行。
 *
 * 外层用 div 而非 label：Select 是自定义弹层组件，label 的
 * 点击代理会误触发展开。
 *
 * @param props 标签、说明、当前值、选项与更新回调
 * @returns 下拉字段行
 */
export function SelectFieldRow<Value extends string>({
  label,
  hint,
  value,
  options,
  onChange
}: FieldRowBaseProps & {
  value: Value;
  options: SelectOption<Value>[];
  onChange: (value: Value) => void;
}) {
  return (
    <div className="settings-field">
      <span>{label}</span>
      <Select value={value} options={options} onChange={onChange} ariaLabel={label} />
      {hint != null && hint !== "" && <small>{hint}</small>}
    </div>
  );
}
