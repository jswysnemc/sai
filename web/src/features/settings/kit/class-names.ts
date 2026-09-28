/**
 * 拼接条件类名，忽略假值片段。
 *
 * @param parts 类名片段；false、null、undefined 与空串会被跳过
 * @returns 以空格连接的类名
 */
export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}
