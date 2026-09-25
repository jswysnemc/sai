const EXPANSION_KEY = "sai.file-tree.expanded";
const MAX_PATHS = 300;

/**
 * 解析某个工作区记下的展开目录。
 *
 * @param raw localStorage 中的 JSON
 * @param workspaceKey 工作区 ID
 * @returns 目录路径集合；内容损坏时为空
 */
export function parseExpandedDirectories(raw: string | null, workspaceKey: string): Set<string> {
  if (!raw) return new Set();
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return new Set();
    const paths = (parsed as Record<string, unknown>)[workspaceKey];
    if (!Array.isArray(paths)) return new Set();
    return new Set(paths.filter((item): item is string => typeof item === "string" && item.length > 0).slice(0, MAX_PATHS));
  } catch {
    return new Set();
  }
}

/**
 * 读取上次展开的目录。
 *
 * @param workspaceKey 工作区 ID
 * @returns 目录路径集合
 */
export function readExpandedDirectories(workspaceKey: string): Set<string> {
  try {
    return parseExpandedDirectories(globalThis.localStorage?.getItem(EXPANSION_KEY) ?? null, workspaceKey);
  } catch {
    return new Set();
  }
}

/**
 * 记住一个工作区的展开目录，其它工作区的记录保留。
 *
 * @param workspaceKey 工作区 ID
 * @param paths 已展开的目录
 */
export function writeExpandedDirectories(workspaceKey: string, paths: ReadonlySet<string>): void {
  try {
    const raw = globalThis.localStorage?.getItem(EXPANSION_KEY);
    let store: Record<string, string[]> = {};
    if (raw) {
      const parsed = JSON.parse(raw) as unknown;
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) store = { ...(parsed as Record<string, string[]>) };
    }
    store[workspaceKey] = [...paths].filter((path) => path.length > 0).slice(0, MAX_PATHS);
    globalThis.localStorage?.setItem(EXPANSION_KEY, JSON.stringify(store));
  } catch {
    // 隐私模式拒绝写入时，展开状态只保留在当前页面
  }
}
