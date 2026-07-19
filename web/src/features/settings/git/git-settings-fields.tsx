import type { ReactNode } from "react";
import { Select, type SelectOption } from "../../../shared/ui/select/select";

type GitSettingToggleProps = {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
};

/**
 * 渲染 Git 设置布尔开关。
 *
 * @param props 标签、说明、当前值和更新回调
 * @returns 开关字段
 */
export function GitSettingToggle(props: GitSettingToggleProps) {
  return (
    <label className="git-setting-toggle">
      <span><strong>{props.label}</strong><small>{props.description}</small></span>
      <input type="checkbox" checked={props.checked} onChange={(event) => props.onChange(event.target.checked)} />
    </label>
  );
}

type GitSettingSelectProps<Value extends string> = {
  label: string;
  description: string;
  value: Value;
  options: SelectOption<Value>[];
  onChange: (value: Value) => void;
};

/**
 * 渲染 Git 设置枚举下拉字段。
 *
 * @param props 标签、说明、当前值、选项和更新回调
 * @returns 枚举字段
 */
export function GitSettingSelect<Value extends string>(props: GitSettingSelectProps<Value>) {
  return (
    <div className="settings-field">
      <span>{props.label}</span>
      <Select value={props.value} options={props.options} onChange={props.onChange} ariaLabel={props.label} />
      <small>{props.description}</small>
    </div>
  );
}

type GitSettingNumberProps = {
  label: string;
  description: ReactNode;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
};

/**
 * 渲染 Git 设置数值字段并限制输入范围。
 *
 * @param props 标签、说明、范围、当前值和更新回调
 * @returns 数值字段
 */
export function GitSettingNumber(props: GitSettingNumberProps) {
  return (
    <label className="settings-field">
      <span>{props.label}</span>
      <input
        type="number"
        min={props.min}
        max={props.max}
        value={props.value}
        onChange={(event) => props.onChange(Math.min(props.max, Math.max(props.min, Number(event.target.value))))}
      />
      <small>{props.description}</small>
    </label>
  );
}
