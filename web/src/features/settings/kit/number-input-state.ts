/** 数字输入的取值约束。 */
export type NumberBounds = {
  min?: number;
  max?: number;
  /** 只接受整数 */
  integer?: boolean;
};

/** 输入草稿的解析结果。 */
export type NumberDraftResult =
  | { kind: "empty" }
  | { kind: "invalid" }
  | { kind: "value"; value: number; inRange: boolean };

/**
 * 解析数字输入草稿。
 *
 * @param draft 输入框中的原始文本
 * @param bounds 取值约束
 * @returns 空、非法或数值（附带是否在范围内）
 */
export function parseNumberDraft(draft: string, bounds: NumberBounds): NumberDraftResult {
  const trimmed = draft.trim();
  if (!trimmed) return { kind: "empty" };
  const value = Number(trimmed);
  // 1. 非数字与不满足整数约束的输入都视为非法，等待继续输入或失焦回退
  if (!Number.isFinite(value)) return { kind: "invalid" };
  if (bounds.integer && !Number.isInteger(value)) return { kind: "invalid" };
  // 2. 合法数值额外标明是否落在范围内，范围外的值只在失焦时夹紧提交
  const inRange = (bounds.min === undefined || value >= bounds.min)
    && (bounds.max === undefined || value <= bounds.max);
  return { kind: "value", value, inRange };
}

/**
 * 把数值夹紧到约束范围内。
 *
 * @param value 原始数值
 * @param bounds 取值约束
 * @returns 范围内的数值；整数约束下四舍五入
 */
export function clampNumber(value: number, bounds: NumberBounds): number {
  let next = bounds.integer ? Math.round(value) : value;
  if (bounds.min !== undefined) next = Math.max(bounds.min, next);
  if (bounds.max !== undefined) next = Math.min(bounds.max, next);
  return next;
}

/**
 * 把数值格式化为输入框文本。
 *
 * @param value 当前数值；空值与非有限数显示为空串
 * @returns 输入框文本
 */
export function formatNumberValue(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) return "";
  return String(value);
}

/** 失焦提交的判定结果。 */
export type NumberCommit = {
  /** 需要提交的值；undefined 表示保持原值 */
  value: number | null | undefined;
  /** 提交后输入框应显示的文本 */
  display: string;
};

/**
 * 失焦或回车时决定最终提交值。
 *
 * 空输入在允许为空时提交 null，否则回退原值；非法输入回退原值；
 * 超出范围的数值夹紧后提交。
 *
 * @param draft 输入框中的原始文本
 * @param current 当前已提交的数值
 * @param bounds 取值约束
 * @param allowEmpty 是否允许清空
 * @returns 提交值与显示文本
 */
export function resolveNumberCommit(
  draft: string,
  current: number | null | undefined,
  bounds: NumberBounds,
  allowEmpty: boolean
): NumberCommit {
  const parsed = parseNumberDraft(draft, bounds);
  if (parsed.kind === "empty") {
    return allowEmpty
      ? { value: current === null || current === undefined ? undefined : null, display: "" }
      : { value: undefined, display: formatNumberValue(current) };
  }
  if (parsed.kind === "invalid") return { value: undefined, display: formatNumberValue(current) };
  const clamped = clampNumber(parsed.value, bounds);
  return {
    value: clamped === current ? undefined : clamped,
    display: formatNumberValue(clamped)
  };
}

/**
 * 按单位文字估算后缀所需宽度，使数值不会被单位遮挡。
 *
 * @param unit 单位文字
 * @returns CSS 长度（rem）
 */
export function unitWidth(unit: string): string {
  let width = 0.25;
  for (const char of unit) width += /[\u3000-\u9fff\uff00-\uffef]/.test(char) ? 0.75 : 0.45;
  return `${width.toFixed(2)}rem`;
}
