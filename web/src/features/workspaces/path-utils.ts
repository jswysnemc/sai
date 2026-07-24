/**
 * 判断是否为可直接跳转的绝对路径（POSIX 或 Windows 盘符/UNC）。
 *
 * @param value 用户输入路径
 * @returns 是否绝对路径
 */
export function isAbsoluteFilesystemPath(value: string): boolean {
  const text = value.trim();
  if (!text) return false;
  if (text.startsWith("/")) return true;
  // Windows 盘符：C:\ 或 C:/
  if (/^[A-Za-z]:[\\/]/.test(text)) return true;
  // UNC：\\server\share 或 //server/share
  if (text.startsWith("\\\\") || text.startsWith("//")) return true;
  return false;
}

/**
 * 规范化路径输入：修剪空白，保留用户分隔符风格。
 *
 * @param value 原始输入
 * @returns 修剪后的路径
 */
export function normalizePathInput(value: string): string {
  return value.trim();
}
