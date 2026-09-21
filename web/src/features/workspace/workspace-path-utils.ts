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
  const normalizedWorkspace = trimTrailingSeparator(workspace);
  if (!normalizedWorkspace) return normalizedPath.replace(/^\.\//, "");
  const pathKey = pathComparisonKey(normalizedPath);
  const workspaceKey = pathComparisonKey(normalizedWorkspace);
  if (pathKey === workspaceKey) return "";
  if (pathKey.startsWith(`${workspaceKey}/`)) {
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
  const value = path.trim().replace(/^\\\\\?\\/, "").replace(/^\/\/\?\//, "").replace(/\\/g, "/");
  const prefix = value.startsWith("//") ? "//" : "";
  return `${prefix}${value.slice(prefix.length).replace(/\/{2,}/g, "/")}`;
}

/**
 * 去掉目录末尾分隔符，但保留 Unix 根目录和 Windows 盘符根目录。
 *
 * @param path 已归一化的路径
 * @returns 可用于路径边界比较的路径
 */
function trimTrailingSeparator(path: string): string {
  if (path === "/" || /^[a-z]:\/$/iu.test(path)) return path;
  return path.replace(/\/+$/u, "");
}

/**
 * 生成路径比较键；Windows 盘符和 UNC 路径按不区分大小写处理。
 *
 * @param path 已归一化的路径
 * @returns 路径比较键
 */
function pathComparisonKey(path: string): string {
  return /^(?:[a-z]:\/|\/\/)/iu.test(path) ? path.toLowerCase() : path;
}

/**
 * 判断路径是否为服务器绝对路径，兼容 Unix、Windows 盘符与网络路径。
 * @param path 文件路径
 * @returns 是否为绝对路径
 */
export function isAbsoluteFilePath(path: string): boolean {
  return /^(?:\/|[a-z]:[\\/]|\\\\)/i.test(path.trim());
}
