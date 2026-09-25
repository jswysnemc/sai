const EXPANSION_KEY = "sai.sidebar.workspace-expansion";

/**
 * 解析已展开的工作区。
 *
 * @param raw localStorage 中的 JSON
 * @returns 工作区 ID 集合；内容损坏时为空
 */
export function parseExpandedWorkspaces(raw: string | null): Set<string> {
  if (!raw) return new Set();
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return new Set();
    return new Set(parsed.filter((item): item is string => typeof item === "string" && item.length > 0));
  } catch {
    return new Set();
  }
}

/**
 * 序列化展开状态。
 *
 * @param ids 已展开的工作区
 * @returns JSON 数组
 */
export function serializeExpandedWorkspaces(ids: ReadonlySet<string>): string {
  return JSON.stringify([...ids]);
}

/**
 * 切换一个工作区的展开状态。
 *
 * @param ids 当前展开集合
 * @param workspaceId 工作区 ID
 * @returns 新集合
 */
export function toggleWorkspaceExpanded(ids: ReadonlySet<string>, workspaceId: string): Set<string> {
  const next = new Set(ids);
  if (next.has(workspaceId)) next.delete(workspaceId);
  else next.add(workspaceId);
  return next;
}

/**
 * 活动工作区保持展开，方便直接看到当前会话。
 *
 * @param ids 当前展开集合
 * @param activeWorkspaceId 活动工作区；没有时原样返回
 * @returns 包含活动工作区的集合
 */
export function withActiveWorkspaceExpanded(ids: ReadonlySet<string>, activeWorkspaceId: string | undefined): Set<string> {
  if (!activeWorkspaceId || ids.has(activeWorkspaceId)) return new Set(ids);
  const next = new Set(ids);
  next.add(activeWorkspaceId);
  return next;
}

/**
 * 读取上次展开的工作区。
 *
 * @returns 工作区 ID 集合
 */
export function readExpandedWorkspaces(): Set<string> {
  try {
    return parseExpandedWorkspaces(globalThis.localStorage?.getItem(EXPANSION_KEY) ?? null);
  } catch {
    return new Set();
  }
}

/**
 * 记住展开的工作区。
 *
 * @param ids 工作区 ID 集合
 */
export function writeExpandedWorkspaces(ids: ReadonlySet<string>) {
  try {
    globalThis.localStorage?.setItem(EXPANSION_KEY, serializeExpandedWorkspaces(ids));
  } catch {
    // 隐私模式拒绝写入时，展开状态只保留在当前页面
  }
}
