/**
 * 把字节数格式化为紧凑的二进制容量文本。
 *
 * @param bytes 原始字节数
 * @returns 容量文本
 */
export function formatSessionBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KiB", "MiB", "GiB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit + 1 < units.length) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 10 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[unit]}`;
}

/**
 * 把 ISO 时间格式化为当前界面的短日期时间。
 *
 * @param value ISO 时间文本
 * @param locale 界面语言
 * @returns 本地日期时间；无效输入返回原文
 */
export function formatSessionDate(value: string, locale: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(locale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit"
  }).format(date);
}
