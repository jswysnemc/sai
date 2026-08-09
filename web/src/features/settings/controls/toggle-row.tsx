import type { ReactNode } from "react";

type ToggleRowProps = {
  /** 主标签 */
  label: ReactNode;
  /** 弱化说明；缺省不渲染 */
  hint?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
};

/**
 * 渲染标准开关行：主标签 + 弱化说明 + 复选框。
 *
 * 设置页所有「布尔字段」共用此结构，样式由 .settings-toggle-field 承载。
 *
 * @param props 标签、说明、当前值与更新回调
 * @returns 开关行
 */
export function ToggleRow({ label, hint, checked, onChange }: ToggleRowProps) {
  return (
    <label className="settings-toggle-field">
      <span>
        <strong>{label}</strong>
        {hint != null && hint !== "" && <small>{hint}</small>}
      </span>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
}
