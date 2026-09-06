import { useQuery } from "@tanstack/react-query";
import { apiRequest } from "../../api/client";

export type RunningSession = { workspace_id: string; session_id: string };
const EMPTY_RUNNING_SESSIONS: ReadonlySet<string> = new Set();

/**
 * 使用工作区与会话共同标识运行状态，防止跨工作区误判。
 * @param workspaceId 工作区标识
 * @param sessionId 会话标识
 * @returns 稳定查询键
 */
export function sessionActivityKey(workspaceId: string, sessionId: string): string {
  return JSON.stringify([workspaceId, sessionId]);
}

/**
 * 将服务端实际运行列表转换为会话索引。
 * @param sessions 持有有效运行锁的会话
 * @returns 正在运行的会话键集合
 */
export function indexRunningSessions(sessions: RunningSession[]): ReadonlySet<string> {
  return new Set(sessions.map((session) => sessionActivityKey(session.workspace_id, session.session_id)));
}

/**
 * 订阅网页、终端和网关共享的实际运行状态。
 * @returns 正在工作的会话集合；查询失败时不展示过期绿点
 */
export function useRunningSessions(): ReadonlySet<string> {
  const query = useQuery({
    queryKey: ["session-activity"],
    queryFn: () => apiRequest<RunningSession[]>("/api/sessions/activity"),
    select: indexRunningSessions,
    refetchInterval: 3_000
  });
  return query.isError ? EMPTY_RUNNING_SESSIONS : query.data ?? EMPTY_RUNNING_SESSIONS;
}
