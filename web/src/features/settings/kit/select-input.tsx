import { Select, type SelectOption } from "../../../shared/ui/select/select";
import { cx } from "./class-names";
import { useFieldContext } from "./field-context";
import "./inputs.css";

export type { SelectOption } from "../../../shared/ui/select/select";

type SkSelectProps<T extends string> = {
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  disabled?: boolean;
  /** 不在字段内使用时必须提供可访问名称 */
  ariaLabel?: string;
  menuPreferredWidth?: number;
  menuMinimumWidth?: number;
  className?: string;
};

/**
 * 渲染设置页下拉选择：沿用共享 Select，高度对齐设置页控件档位，
 * 并自动取所在字段的标签作为可访问名称。
 *
 * @param props 当前值、选项、更新回调与弹层宽度
 * @returns 下拉选择
 */
export function SkSelect<T extends string>({
  value,
  options,
  onChange,
  disabled,
  ariaLabel,
  menuPreferredWidth,
  menuMinimumWidth,
  className
}: SkSelectProps<T>) {
  const field = useFieldContext();
  return (
    <Select
      id={field?.controlId}
      className={cx("sk-select", className)}
      value={value}
      options={options}
      onChange={onChange}
      disabled={disabled}
      ariaLabel={ariaLabel ?? field?.labelText}
      menuPreferredWidth={menuPreferredWidth}
      menuMinimumWidth={menuMinimumWidth}
    />
  );
}
