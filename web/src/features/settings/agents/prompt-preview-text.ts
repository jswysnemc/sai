/**
 * 【Agent】【提示词预览】移除高亮标记，仅生成安全文本，不修改存储内容。
 * @param prompt 原始提示词
 * @returns 用于 React 文本节点的预览内容
 */
export function promptPreviewText(prompt: string): string {
  return prompt.replace(/<\/?mark\b[^>]*>/gi, "");
}
