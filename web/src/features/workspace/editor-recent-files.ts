const STORAGE_KEY = "sai.editor.recent-files";
const LIMIT = 12;

/**
 * 读取最近打开的工作区文件，最新的在前。
 *
 * @returns 相对路径列表；没有记录时为空
 */
export function readRecentFiles(): string[] {
  try {
    const raw = JSON.parse(globalThis.localStorage?.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(raw) ? raw.filter((item): item is string => typeof item === "string" && item.length > 0).slice(0, LIMIT) : [];
  } catch {
    return [];
  }
}

/**
 * 把刚打开的文件记到最近列表最前面。
 *
 * @param path 工作区相对路径
 * @returns 更新后的列表
 */
export function rememberRecentFile(path: string): string[] {
  const next = [path, ...readRecentFiles().filter((item) => item !== path)].slice(0, LIMIT);
  try {
    globalThis.localStorage?.setItem(STORAGE_KEY, JSON.stringify(next));
  } catch {
    // 浏览器限制存储时只保留本次返回值
  }
  return next;
}
