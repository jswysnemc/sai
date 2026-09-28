import { SegmentedControl, type SegmentedControlOption } from "../../../shared/ui/segmented-control";
import { cx } from "./class-names";
import { useFieldContext } from "./field-context";
import "./choice-pills.css";

type ChoicePillsProps<T extends string> = {
  value: T;
  options: readonly SegmentedControlOption<T>[];
  onChange: (value: T) => void;
  /** 不在字段内使用时必须提供可访问名称 */
  ariaLabel?: string;
  /** 选项文字使用等宽字体，适合 low / medium / high 这类取值 */
  mono?: boolean;
  className?: string;
};

/**
 * 渲染紧凑分段选择，用于 5 项以内的枚举字段。
 *
 * @param props 当前值、选项、更新回调与可访问名称
 * @returns 分段选择
 */
export function ChoicePills<T extends string>({ value, options, onChange, ariaLabel, mono, className }: ChoicePillsProps<T>) {
  const field = useFieldContext();
  return (
    <SegmentedControl
      value={value}
      options={options}
      onChange={onChange}
      ariaLabel={ariaLabel ?? field?.labelText ?? ""}
      className={cx("sk-pills", mono && "is-mono", className)}
    />
  );
}
