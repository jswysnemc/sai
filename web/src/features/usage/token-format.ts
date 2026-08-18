/**
 * 格式化较大的计数。
 *
 * @param value 原始数值
 * @returns 紧凑计数文本，如 998.6k、1.2m
 */
export function formatTokenCount(value: number): string {
  if (value >= 1_000_000) return `${stripTrailingZero(value / 1_000_000)}m`;
  if (value >= 1_000) return `${stripTrailingZero(value / 1_000)}k`;
  return String(value);
}

/**
 * 移除一位小数格式中的无效零。
 *
 * @param value 需要压缩显示的数值
 * @returns 最多保留一位小数的文本
 */
function stripTrailingZero(value: number): string {
  return value.toFixed(1).replace(/\.0$/, "");
}
