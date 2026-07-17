/**
 * 把文件路径转换为工作空间相对路径。
 *
 * @param path 待转换的文件路径
 * @param workspacePath 当前工作空间绝对路径
 * @returns 工作空间相对路径；无法匹配工作空间时返回清理后的原路径
 */
export function workspaceRelativePath(path: string, workspacePath: string): string {
  const normalizedPath = normalizePathSeparators(path);
  const workspace = normalizePathSeparators(workspacePath);
  const normalizedWorkspace = workspace.length > 1 ? workspace.replace(/\/$/, "") : workspace;
  if (!normalizedWorkspace) return normalizedPath.replace(/^\.\//, "");
  if (normalizedPath === normalizedWorkspace) return "";
  if (normalizedPath.startsWith(`${normalizedWorkspace}/`)) {
    return normalizedPath.slice(normalizedWorkspace.length + 1);
  }
  return normalizedPath.replace(/^\.\//, "");
}

/**
 * 统一路径分隔符并清理重复斜线。
 *
 * @param path 待处理路径
 * @returns 使用正斜线的路径
 */
function normalizePathSeparators(path: string): string {
  return path.trim().replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "").replace(/\\/g, "/").replace(/\/{2,}/g, "/");
}
