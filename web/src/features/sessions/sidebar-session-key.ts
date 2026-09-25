/** 复合键分隔符。工作区 ID 与会话 ID 都不会包含这个字符。 */
const SESSION_KEY_SEPARATOR = "\u001f";

/**
 * 生成跨工作区稳定的会话键。
 *
 * 多个工作区都有 id 为 `default` 的会话，裸 id 会在列表、置顶和未读里互相覆盖。
 *
 * @param workspaceId 工作区 ID
 * @param sessionId 会话 ID
 * @returns 复合键
 */
export function sidebarSessionKey(workspaceId: string, sessionId: string): string {
  return `${workspaceId}${SESSION_KEY_SEPARATOR}${sessionId}`;
}

/**
 * 统计同一个会话 ID 出现了几次。
 *
 * @param sessionIds 当前列表里的会话 ID
 * @returns 每个 ID 的出现次数
 */
export function countSessionIds(sessionIds: readonly string[]): Map<string, number> {
  const counts = new Map<string, number>();
  for (const sessionId of sessionIds) {
    counts.set(sessionId, (counts.get(sessionId) ?? 0) + 1);
  }
  return counts;
}

/**
 * 判断索引里存的标识是否指向这一条会话。
 *
 * 新数据用复合键。旧数据只有会话 ID，并且只在全局唯一时还能对应上。
 *
 * @param storedId 索引中的标识
 * @param workspaceId 工作区 ID
 * @param sessionId 会话 ID
 * @param sameIdCount 该会话 ID 在全部工作区中的数量
 * @returns 指向这一条时为 true
 */
export function matchesStoredSessionId(
  storedId: string,
  workspaceId: string,
  sessionId: string,
  sameIdCount: number
): boolean {
  if (storedId === sidebarSessionKey(workspaceId, sessionId)) return true;
  return sameIdCount <= 1 && storedId === sessionId;
}

/**
 * 写入置顶时改成复合键，并去掉会波及其他工作区的裸 ID。
 *
 * @param pinned 当前置顶标识
 * @param workspaceId 工作区 ID
 * @param sessionId 会话 ID
 * @param pinnedNow 当前是否已置顶
 * @returns 新的置顶列表
 */
export function nextPinnedIds(
  pinned: readonly string[],
  workspaceId: string,
  sessionId: string,
  pinnedNow: boolean
): string[] {
  const key = sidebarSessionKey(workspaceId, sessionId);
  const rest = pinned.filter((id) => id !== key && id !== sessionId);
  return pinnedNow ? rest : [key, ...rest];
}
