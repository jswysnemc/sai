import type { ReactNode } from "react";

type ToggleRowProps = {
  /** 主标签 */
  label: ReactNode;
  /** 弱化说明；缺省不渲染 */
  hint?: ReactNode;
  checked: boolean;
  onChange: (checked: boolean) => void;
  /** 行容器类名；缺省走设置卡片行，传入后不再套用卡片栅格 */
  className?: string;
};

/**
 * 渲染标准开关行：主标签 + 弱化说明 + 复选框。
 *
 * 设置页所有「布尔字段」共用此结构。缺省样式由 .settings-toggle-field 承载；
 * 传入 className 时改由调用方控制行布局，开关本体仍使用 .switch-control。
 *
 * @param props 标签、说明、当前值、更新回调，以及可选行容器类名
 * @returns 开关行
 */
export function ToggleRow({ label, hint, checked, onChange, className = "settings-toggle-field" }: ToggleRowProps) {
  return (
    <label className={className}>
      <span>
        <strong>{label}</strong>
        {hint != null && hint !== "" && <small>{hint}</small>}
      </span>
      <input
        className="switch-control"
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
    </label>
  );
}
