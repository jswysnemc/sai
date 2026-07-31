import type { ToolMode } from "./agent-tool-mode-state";

/** 三段切换的单个档位 */
type ModeOption = {
  value: ToolMode;
  label: string;
  title: string;
};

type ToolModeSwitchProps = {
  /** 当前状态 */
  value: ToolMode;
  /** 三个档位的文案 */
  options: ModeOption[];
  /** 可访问名称 */
  ariaLabel: string;
  /** 状态变化回调 */
  onChange: (mode: ToolMode) => void;
};

/**
 * 渲染 on / load / off 三段切换控件。
 *
 * 用分段按钮而非下拉，三个档位同时可见，扫一眼就能看出当前处于哪一段。
 *
 * @param props 当前状态、档位文案与变化回调
 * @returns 三段切换控件
 */
export function ToolModeSwitch({ value, options, ariaLabel, onChange }: ToolModeSwitchProps) {
  return (
    <div className="agent-tool-mode-switch" role="radiogroup" aria-label={ariaLabel}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          data-mode={option.value}
          data-active={value === option.value ? "true" : "false"}
          title={option.title}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}
