import type { ReactNode } from "react";
import { fieldAnchorId } from "../search/field-anchor";
import { cx } from "./class-names";
import "./switch.css";

type SwitchProps = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  id?: string;
  ariaLabel?: string;
  ariaDescribedBy?: string;
};

/**
 * 渲染紧凑开关，语义为 role="switch" 的复选框。
 *
 * @param props 选中状态、更新回调与无障碍属性
 * @returns 开关控件
 */
export function Switch({ checked, onChange, disabled, id, ariaLabel, ariaDescribedBy }: SwitchProps) {
  return (
    <input
      type="checkbox"
      role="switch"
      className="sk-switch"
      id={id}
      checked={checked}
      disabled={disabled}
      aria-label={ariaLabel}
      aria-describedby={ariaDescribedBy}
      onChange={(event) => onChange(event.target.checked)}
    />
  );
}

type InlineSwitchProps = {
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** 开关后的状态文字 */
  label: string;
  disabled?: boolean;
};

/**
 * 渲染带状态文字的行内开关，用于详情标题等操作区。
 *
 * @param props 选中状态、状态文字与更新回调
 * @returns 行内开关
 */
export function InlineSwitch({ checked, onChange, label, disabled }: InlineSwitchProps) {
  return (
    <label className="sk-inline-switch">
      <Switch checked={checked} onChange={onChange} disabled={disabled} />
      <span>{label}</span>
    </label>
  );
}

type SwitchFieldProps = {
  label: ReactNode;
  hint?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  /** 对应的配置键，只出现在标签提示与搜索索引中 */
  configKey?: string;
  /** 搜索定位锚点 */
  anchor?: string;
  span?: "full";
  /** 紧跟标签的徽标 */
  badge?: ReactNode;
  className?: string;
};

/**
 * 渲染前置开关字段：开关在前，标签与说明在后，整行可点击。
 *
 * @param props 标签、说明、选中状态、配置键与锚点
 * @returns 开关字段
 */
export function SwitchField({
  label,
  hint,
  checked,
  onChange,
  disabled,
  configKey,
  anchor,
  span,
  badge,
  className
}: SwitchFieldProps) {
  return (
    <label
      className={cx("sk-switch-field", span === "full" && "is-full", disabled && "is-disabled", className)}
      id={anchor ? fieldAnchorId(anchor) : undefined}
      data-config-key={configKey}
      title={configKey}
    >
      <Switch checked={checked} onChange={onChange} disabled={disabled} />
      <span className="sk-switch-copy">
        <span className="sk-switch-label">{label}{badge}</span>
        {hint && <span className="sk-switch-hint">{hint}</span>}
      </span>
    </label>
  );
}
