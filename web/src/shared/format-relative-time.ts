/** 相对当前时刻的时间文案（会话侧栏等）。 */

/**
 * 将时间戳格式化为相对当前时刻的短文案。
 *
 * @param value ISO 字符串或可解析时间
 * @param locale 语言
 * @param nowMs 当前毫秒时间戳，便于测试注入
 * @returns 如「刚刚」「5 分钟前」「昨天」「3/12」
 */
export function formatRelativeTime(
  value: string | number | Date,
  locale: string = "zh-CN",
  nowMs: number = Date.now(),
): string {
  const date = value instanceof Date ? value : new Date(value);
  const ts = date.getTime();
  if (!Number.isFinite(ts)) return "";

  const diffSec = Math.round((ts - nowMs) / 1000);
  const abs = Math.abs(diffSec);
  const rtf = new Intl.RelativeTimeFormat(locale === "zh-CN" ? "zh-CN" : "en", {
    numeric: "auto",
  });

  // 1. 1 分钟内
  if (abs < 60) {
    return locale.startsWith("zh") ? "刚刚" : "just now";
  }
  // 2. 分钟
  if (abs < 3600) {
    return rtf.format(Math.trunc(diffSec / 60), "minute");
  }
  // 3. 小时
  if (abs < 86_400) {
    return rtf.format(Math.trunc(diffSec / 3600), "hour");
  }
  // 4. 天（一周内）
  if (abs < 86_400 * 7) {
    return rtf.format(Math.trunc(diffSec / 86_400), "day");
  }
  // 5. 更久：同年显示月日，跨年带年份
  const now = new Date(nowMs);
  const sameYear = date.getFullYear() === now.getFullYear();
  return date.toLocaleDateString(locale.startsWith("zh") ? "zh-CN" : "en-US", {
    month: "numeric",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}
