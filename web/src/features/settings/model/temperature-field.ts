/** 温度写入配置时保留的小数位，避免 f32 回读成 0.8999999761581421。 */
const TEMPERATURE_DECIMALS = 4;

/**
 * 把配置里的温度格式化为输入框文本。
 *
 * @param value 配置中的温度；缺省表示不发送该参数
 * @returns 可展示的十进制文本；缺省为空串
 */
export function formatTemperature(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value)) return "";
  const rounded = roundTemperature(value);
  return String(rounded);
}

/**
 * 解析温度输入。空串表示不发送；越界或非数字视为非法。
 *
 * @param raw 输入框文本
 * @returns 合法时给出数值或 undefined；非法时 ok 为 false
 */
export function parseTemperature(
  raw: string
): { ok: true; value: number | undefined } | { ok: false } {
  const trimmed = raw.trim();
  if (!trimmed) return { ok: true, value: undefined };
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed) || parsed < 0 || parsed > 2) return { ok: false };
  return { ok: true, value: roundTemperature(parsed) };
}

/**
 * 按固定小数位四舍五入，保证写入与回显一致。
 *
 * @param value 原始温度
 * @returns 四舍五入后的温度
 */
export function roundTemperature(value: number): number {
  const scale = 10 ** TEMPERATURE_DECIMALS;
  return Math.round(value * scale) / scale;
}
