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
export function useSessionTree() {
  const tree = useQuery({
    queryKey: ["session-tree"],
    queryFn: api.sessions.tree,
    refetchInterval: 3000,
    select: (workspaces) => workspaces.map((workspace) => ({
      ...workspace,
      sessions: workspace.sessions.filter((session) => !isSideConversationSessionTitle(session.title))
    }))
  });

  return { tree };
}
