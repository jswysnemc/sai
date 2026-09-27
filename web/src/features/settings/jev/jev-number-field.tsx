import { useEffect, useState } from "react";
import { clampNumber } from "./jev-config";

type JevNumberFieldProps = {
  label: string;
  hint?: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  integer?: boolean;
  onChange: (value: number) => void;
};

/**
 * 【Jev设置】【数值字段】输入过程中保留原始文本，失焦时再收敛到范围内，避免输入小数时被提前截断。
 * @param props 标签、说明、范围与更新回调
 * @returns 数值字段行
 */
export function JevNumberField({ label, hint, value, min, max, step = 1, integer = false, onChange }: JevNumberFieldProps) {
  const [text, setText] = useState(String(value));

  // 外部值变化（如切换接入、撤销）时同步输入框
  useEffect(() => setText(String(value)), [value]);

  /** 失焦时提交范围内的数值。 */
  const commit = () => {
    const next = clampNumber(text, min, max, integer);
    setText(String(next));
    if (next !== value) onChange(next);
  };

  return (
    <label className="settings-field">
      <span>{label}</span>
      <input type="number" inputMode="decimal" min={min} max={max} step={step} value={text} onChange={(event) => setText(event.target.value)} onBlur={commit} onKeyDown={(event) => { if (event.key === "Enter") commit(); }} />
      {hint && <small>{hint}</small>}
    </label>
  );
}
