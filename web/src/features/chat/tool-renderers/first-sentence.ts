/**
 * 把说明压成一句，避免折叠行铺开整段描述。
 *
 * 中文句号、叹号、问号后面可以没有空格。英文句号仍要求后面是空白，避免切开 `1.2` 这类数字。
 *
 * @param value 原始说明
 * @returns 首句；超过 140 字时截断并加省略号
 */
export function firstSentence(value: string): string {
  const single = value.replace(/\s+/g, " ").trim();
  // 1. 在句末标点处切开，标点留在首句
  const sentence = single.split(/(?<=[。！？])|(?<=[.!?])(?=\s)/u)[0]?.trim() ?? single;
  if (sentence.length <= 140) return sentence;
  return `${sentence.slice(0, 139)}…`;
}
