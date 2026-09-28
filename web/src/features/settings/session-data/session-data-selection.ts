import type { SessionDataSelection, SessionDataSummary } from "../../../api/contracts";

/**
 * 【会话数据】【定位】生成跨工作区稳定选择键。
 * @param session 会话摘要
 * @returns 同名会话不会冲突的选择键
 */
export function sessionKey(session: SessionDataSummary): string {
  return `${session.workspace_id}/${session.id}`;
}

/**
 * 将会话摘要转换为带工作区的批量请求项。
 * @param session 会话摘要
 * @returns API 请求项
 */
export function toSelection(session: SessionDataSummary): SessionDataSelection {
  return { workspace_id: session.workspace_id, session_id: session.id };
}

/**
 * 【会话数据】【筛选】按工作区、名称、时间与最小存储大小筛选。
 * @param items 会话摘要
 * @param query 名称、路径或日期关键词
 * @param workspaceId 工作区标识，空值表示全部
 * @param minBytes 最小存储大小
 * @returns 可见会话列表
 */
export function filterSessionData(items: readonly SessionDataSummary[], query: string, workspaceId: string, minBytes: number): SessionDataSummary[] {
  const needle = query.trim().toLowerCase();
  return items.filter((item) => (!workspaceId || item.workspace_id === workspaceId)
    && item.total_bytes >= minBytes
    && [item.title, item.id, item.workspace_name, item.workspace_path, item.updated_at].some((value) => value.toLowerCase().includes(needle)));
}

/**
 * 判断会话是否明确处于可清理状态。
 * @param session 会话摘要
 * @returns 服务端明确上报空闲时为真，未知状态不参与批量清理
 */
export function isIdleSession(session: SessionDataSummary): boolean {
  return session.busy === false;
}
