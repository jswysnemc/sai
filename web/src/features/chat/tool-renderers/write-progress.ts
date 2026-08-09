/*
 * 写入进度估算
 *
 * 编辑类工具执行时参数以 JSON 文本流式抵达，其中换行以 `\n` 转义序列
 * 出现。统计转义换行数即可近似"已经写到第几行"，供折叠行的
 * 数字动效使用，不必等参数闭合成合法 JSON。
 */

/**
 * 统计参数流中已出现的转义换行数。
 *
 * @param argumentsText 工具参数原始文本（可能是不完整的 JSON 前缀）
 * @returns 转义换行数量，可视作已写入的行数
 */
export function streamedLineCount(argumentsText: string): number {
  if (!argumentsText) return 0;
  let count = 0;
  let index = argumentsText.indexOf("\\n");
  while (index !== -1) {
    count += 1;
    index = argumentsText.indexOf("\\n", index + 2);
  }
  return count;
}
