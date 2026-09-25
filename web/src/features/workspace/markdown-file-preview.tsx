/**
 * 判断文件是否按 Markdown 编辑。
 *
 * @param path 文件路径
 * @returns 是否为 Markdown 文件
 */
export function isMarkdownFile(path: string): boolean {
  return /\.(md|markdown)$/i.test(path);
}
