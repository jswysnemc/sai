/**
 * 格式化请求计数。
 *
 * @param value 原始计数
 * @returns 带千分位的整数文本，空值返回 0
 */
export function formatCount(value?: number | null) {
  if (!value || !Number.isFinite(value)) return "0";
  return Math.round(value).toLocaleString();
}

/**
 * 格式化 Token 数量，大数值降级为 K / M 单位。
 *
 * @param value 原始 Token 数
 * @returns 紧凑的数量文本，空值返回 0
 */
export function formatTokens(value?: number | null) {
  const amount = Number(value ?? 0);
  if (!Number.isFinite(amount) || amount <= 0) return "0";
  if (amount >= 1_000_000) return `${(amount / 1_000_000).toFixed(amount >= 10_000_000 ? 1 : 2)}M`;
  if (amount >= 10_000) return `${(amount / 1_000).toFixed(1)}K`;
  return Math.round(amount).toLocaleString();
}

/**
 * 格式化毫秒耗时。
 *
 * @param ms 毫秒数
 * @returns 秒或毫秒文本，空值返回占位符
 */
export function formatDuration(ms?: number | null) {
  const amount = Number(ms ?? 0);
  if (!Number.isFinite(amount) || amount <= 0) return "--";
  if (amount >= 1000) return `${(amount / 1000).toFixed(1)}s`;
  return `${Math.round(amount)}ms`;
}

/**
 * 格式化秒级时间戳。
 *
 * @param seconds 秒级时间戳
 * @param locale 展示语言
 * @returns 月日时分文本，空值返回占位符
 */
export function formatTime(seconds?: number | null, locale: "en-US" | "zh-CN" = "zh-CN") {
  if (!seconds) return "--";
  return new Date(seconds * 1000).toLocaleString(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

/**
 * 计算百分比文本。
 *
 * @param part 分子
 * @param whole 分母
 * @returns 取整后的百分比文本，分母为零时返回 0%
 */
export function formatPercent(part: number, whole: number) {
  if (!whole) return "0%";
  return `${Math.round((part / whole) * 100)}%`;
}

/**
 * 计算原始口径相对计费口径的放大倍数。
 *
 * 缓存命中率高时两者差距可达一个数量级，用倍数直观提示口径差异。
 *
 * @param raw 原始输入量
 * @param billable 等效计费输入量
 * @returns 保留一位小数的倍数文本，无法计算时返回空字符串
 */
export function formatRatio(raw: number, billable: number) {
  if (!billable || !raw || raw <= billable) return "";
  return `${(raw / billable).toFixed(1)}x`;
}
