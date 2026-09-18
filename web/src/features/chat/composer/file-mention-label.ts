/**
 * 提取文件引用的简短显示名称，兼容 Unix 和 Windows 路径。
 *
 * @param path 引用保存的完整文件路径
 * @returns 文件名；路径没有有效名称时返回原值
 */
export function fileMentionLabel(path: string): string {
  return path.replaceAll("\\", "/").split("/").filter(Boolean).at(-1) ?? path;
}
