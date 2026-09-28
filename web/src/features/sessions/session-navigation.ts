import { notifyManager, type QueryClient } from "@tanstack/react-query";
import type { Session, WorkspaceSessions } from "../../api/contracts";

type NavigationState = { version: number; tail: Promise<void>; workspaceId?: string };
type NavigationContext = {
  isCurrent: () => boolean;
  workspaceActive: (id: string, fallback: boolean) => boolean;
  selectWorkspace: (id: string) => void;
};
const navigationByClient = new WeakMap<QueryClient, NavigationState>();

/**
 * 【会话导航】【请求顺序】串行修改服务端指针，仅允许最后一次选择更新界面。
 * @param client 当前页面查询客户端
 * @param action 包含工作区和会话切换的完整操作
 * @returns 本次操作处理完成后的 Promise
 */
export function enqueueSessionNavigation(
  client: QueryClient,
  action: (context: NavigationContext) => Promise<void>
): Promise<void> {
  let state = navigationByClient.get(client);
  if (!state) {
    state = { version: 0, tail: Promise.resolve() };
    navigationByClient.set(client, state);
  }
  const current = state;
  const version = ++current.version;
  const isCurrent = () => current.version === version;
  const operation = current.tail.then(async () => {
    if (!isCurrent()) return;
    await action({
      isCurrent,
      workspaceActive: (id, fallback) => current.workspaceId ? current.workspaceId === id : fallback,
      selectWorkspace: (id) => { current.workspaceId = id; }
    });
  });
  // 1. 【会话导航】【失败恢复】一次切换失败不阻塞后续选择
  current.tail = operation.catch(() => {});
  return operation;
}

/**
 * 【会话导航】【标签页选择】只更新当前查询缓存中的选中标记，不写入服务端共享指针。
 *
 * @param client 当前页面查询客户端
 * @param workspaceId 目标工作区标识
 * @param sessionId 当前标签页选中的会话标识
 * @returns 无返回值
 */
export function commitLocalSessionSelection(client: QueryClient, workspaceId: string, sessionId: string): void {
  notifyManager.batch(() => {
    client.setQueryData<Session[]>(["sessions"], (sessions) => {
      if (!sessions?.some((session) => session.id === sessionId)) return sessions;
      return sessions.map((session) => session.active === (session.id === sessionId) ? session : { ...session, active: session.id === sessionId });
    });
    client.setQueryData<WorkspaceSessions[]>(["session-tree"], (tree) => tree?.map((workspace) => {
      if (workspace.workspace_id !== workspaceId) return workspace;
      return {
        ...workspace,
        sessions: workspace.sessions.map((session) => session.active === (session.id === sessionId) ? session : { ...session, active: session.id === sessionId })
      };
    }));
  });
}

/**
 * 【会话导航】【创建缓存】将已经创建的会话加入所属列表，供界面立即选择。
 * @param client 查询客户端；workspaceId 为所属工作区；session 为创建接口返回的记录
 * @returns 无返回值
 */
export function insertCreatedSession(client: QueryClient, workspaceId: string, session: Session): void {
  notifyManager.batch(() => {
    const tree = client.getQueryData<WorkspaceSessions[]>(["session-tree"]);
    if (tree?.some((workspace) => workspace.workspace_id === workspaceId && workspace.active)) {
      client.setQueryData<Session[]>(["sessions"], (sessions) => [session, ...(sessions ?? []).filter((item) => item.id !== session.id)]);
    }
    client.setQueryData<WorkspaceSessions[]>(["session-tree"], (workspaces) => workspaces?.map((workspace) =>
      workspace.workspace_id === workspaceId
        ? { ...workspace, sessions: [session, ...workspace.sessions.filter((item) => item.id !== session.id)] }
        : workspace));
  });
}
