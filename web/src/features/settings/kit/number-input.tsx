import { useState, type CSSProperties, type KeyboardEvent } from "react";
import { cx } from "./class-names";
import { useFieldContext } from "./field-context";
import {
  clampNumber,
  formatNumberValue,
  parseNumberDraft,
  resolveNumberCommit,
  unitWidth,
  type NumberBounds
} from "./number-input-state";
import "./inputs.css";

type SkNumberInputProps = NumberBounds & {
  value: number | null | undefined;
  onChange: (value: number | null) => void;
  /** 方向键步进值 */
  step?: number;
  /** 输入框内的单位后缀，例如「秒」「ms」 */
  unit?: string;
  /** 允许清空为 null */
  allowEmpty?: boolean;
  placeholder?: string;
  disabled?: boolean;
  ariaLabel?: string;
};

/**
 * 渲染设置页数字输入。
 *
 * 输入过程保留文本草稿，合法且在范围内的数值即时提交；
 * 失焦或回车时夹紧超范围数值、回退非法输入。方向键按步进增减。
 *
 * @param props 数值、约束、单位与更新回调
 * @returns 数字输入框
 */
export function SkNumberInput({
  value,
  onChange,
  min,
  max,
  integer,
  step = 1,
  unit,
  allowEmpty = false,
  placeholder,
  disabled,
  ariaLabel
}: SkNumberInputProps) {
  const field = useFieldContext();
  const bounds: NumberBounds = { min, max, integer };
  const [draft, setDraft] = useState<string | null>(null);
  const display = draft ?? formatNumberValue(value);

  /**
   * 处理输入：合法且在范围内的数值即时提交。
   *
   * @param text 输入框文本
   * @returns 无返回值
   */
  const handleInput = (text: string) => {
    setDraft(text);
    const parsed = parseNumberDraft(text, bounds);
    if (parsed.kind === "empty" && allowEmpty) onChange(null);
    if (parsed.kind === "value" && parsed.inRange) onChange(parsed.value);
  };

  /**
   * 结束编辑：夹紧或回退后退出草稿态。
   *
   * @returns 无返回值
   */
  const commit = () => {
    if (draft === null) return;
    const result = resolveNumberCommit(draft, value, bounds, allowEmpty);
    if (result.value !== undefined) onChange(result.value);
    setDraft(null);
  };

  /**
   * 方向键步进，回车提交。
   *
   * @param event 键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      commit();
      return;
    }
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
    event.preventDefault();
    // 1. 以当前草稿（若合法）或已提交值为基准
    const parsed = parseNumberDraft(display, bounds);
    const base = parsed.kind === "value" ? parsed.value : (value ?? min ?? 0);
    // 2. 步进后夹紧并立即提交，保留草稿态直到失焦
    const next = clampNumber(base + (event.key === "ArrowUp" ? step : -step), bounds);
    const rounded = Number(next.toFixed(6));
    setDraft(formatNumberValue(rounded));
    onChange(rounded);
  };

  const style = unit ? ({ "--sk-unit-width": unitWidth(unit) } as CSSProperties) : undefined;
  return (
    <span className={cx("sk-number", unit && "has-unit")} style={style}>
      <input
        type="text"
        inputMode={integer ? "numeric" : "decimal"}
        className="sk-input"
        id={field?.controlId}
        aria-label={ariaLabel}
        aria-describedby={field?.hintId}
        aria-invalid={field?.invalid || undefined}
        value={display}
        placeholder={placeholder}
        disabled={disabled}
        autoComplete="off"
        spellCheck={false}
        onChange={(event) => handleInput(event.target.value)}
        onBlur={commit}
        onKeyDown={handleKeyDown}
      />
      {unit && <span className="sk-number-unit" aria-hidden="true">{unit}</span>}
    </span>
  );
}
