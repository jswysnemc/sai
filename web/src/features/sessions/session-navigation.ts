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
 * 【会话导航】【选中同步】取消旧列表请求，同时更新消息区和侧栏选中项。
 * @param client 当前页面查询客户端
 * @param workspaceId 目标工作区标识
 * @param selected 服务端确认的目标会话
 * @returns 两份会话缓存同步完成后的 Promise
 */
export async function commitSessionSelection(client: QueryClient, workspaceId: string, selected: Session): Promise<void> {
  await Promise.all([
    client.cancelQueries({ queryKey: ["sessions"] }),
    client.cancelQueries({ queryKey: ["session-tree"] })
  ]);
  notifyManager.batch(() => {
    client.setQueryData<Session[]>(["sessions"], (sessions) => {
      const items = sessions?.some((session) => session.id === selected.id) ? sessions : [...sessions ?? [], selected];
      return items.map((session) => session.id === selected.id ? { ...selected, active: true } : { ...session, active: false });
    });
    client.setQueryData<WorkspaceSessions[]>(["session-tree"], (tree) => tree?.map((workspace) => {
      if (workspace.workspace_id !== workspaceId) return workspace;
      const items = workspace.sessions.some((session) => session.id === selected.id)
        ? workspace.sessions : [...workspace.sessions, selected];
      return { ...workspace, sessions: items.map((session) => session.id === selected.id
        ? { ...selected, active: true } : { ...session, active: false }) };
    }));
  });
}
