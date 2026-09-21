import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { isSideConversationSessionTitle } from "../side-conversation/side-conversation-events";

/**
 * 管理工作区会话树。
 *
 * 周期更新各工作区会话；展示范围和实际运行状态由侧栏单独管理。
 *
 * @returns 会话树查询
 */
export function useSessionTree(selectedSessionId?: string) {
  const sessions = useQuery({ queryKey: ["sessions"], queryFn: api.sessions.list });
  const activeSessionId = sessions.data?.find((session) => session.active)?.id;
  const localSessionId = selectedSessionId && sessions.data?.some((session) => session.id === selectedSessionId)
    ? selectedSessionId
    : activeSessionId;
  const tree = useQuery({
    queryKey: ["session-tree"],
    queryFn: api.sessions.tree,
    refetchInterval: 3000,
    select: (workspaces) => workspaces.map((workspace) => ({
      ...workspace,
      // 1. 【会话同步】【侧栏选择】与消息区共用当前选择，避免独立轮询采用终端的另一份指针快照
      sessions: workspace.sessions.filter((session) => !isSideConversationSessionTitle(session.title)).map((session) =>
        workspace.active && sessions.data ? { ...session, active: session.id === localSessionId } : session)
    }))
  });

  return { tree };
}
