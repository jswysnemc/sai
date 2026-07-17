import type { ReactNode } from "react";

export type SegmentedControlOption<T extends string> = {
  value: T;
  label: string;
  icon?: ReactNode;
};

type SegmentedControlProps<T extends string> = {
  value: T;
  options: readonly SegmentedControlOption<T>[];
  onChange: (value: T) => void;
  ariaLabel: string;
  className?: string;
};

/**
 * 渲染支持键盘方向键切换的单选分段控件。
 *
 * @param props value 为当前值，options 为选项，onChange 为切换回调
 * @returns 可访问的分段切换控件
 */
export function SegmentedControl<T extends string>({ value, options, onChange, ariaLabel, className = "" }: SegmentedControlProps<T>) {
  /** 根据方向键选择相邻选项。 */
  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const current = Math.max(0, options.findIndex((option) => option.value === value));
    const direction = event.key === "ArrowRight" ? 1 : -1;
    const next = (current + direction + options.length) % options.length;
    onChange(options[next].value);
  };

  return (
    <div className={`segmented-control ${className}`.trim()} role="radiogroup" aria-label={ariaLabel} onKeyDown={handleKeyDown}>
      {options.map((option) => (
        <button
          type="button"
          role="radio"
          aria-checked={option.value === value}
          className={option.value === value ? "active" : ""}
          tabIndex={option.value === value ? 0 : -1}
          onClick={() => onChange(option.value)}
          key={option.value}
        >
          {option.icon}
          <span>{option.label}</span>
        </button>
      ))}
    </div>
  );
}
