import { SettingsField, SkNumberInput } from "../kit";

type JevNumberFieldProps = {
  label: string;
  hint?: string;
  anchor?: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  integer?: boolean;
  disabled?: boolean;
  onChange: (value: number) => void;
};

/**
 * 【Jev设置】【数值字段】使用统一数字控件编辑概率、配额与超时。
 * @param props 标签、说明、锚点、范围、当前数值与更新回调
 * @returns 带范围约束的数字字段
 */
export function JevNumberField({ label, hint, anchor, value, onChange, ...bounds }: JevNumberFieldProps) {
  return <SettingsField label={label} hint={hint} anchor={anchor} configKey={anchor} size="sm">
    <SkNumberInput value={value} onChange={(next) => onChange(next ?? value)} {...bounds} />
  </SettingsField>;
}
