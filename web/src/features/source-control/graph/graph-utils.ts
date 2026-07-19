/**
 * 将 Git ISO 时间格式化为当前界面语言的日期时间。
 *
 * @param value Git 日期字符串
 * @param locale 当前界面语言
 * @returns 本地化日期文本
 */
export function formatGitDate(value: string, locale: string): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(locale);
}

/**
 * 清理 Git decorate 文本，使引用标签保持紧凑。
 *
 * @param reference Git decorate 引用
 * @returns 适合界面展示的引用名称
 */
export function formatGitReference(reference: string): string {
  return reference.replace(/^HEAD -> /, "").trim();
}
