import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { isSideConversationSessionTitle } from "../side-conversation/side-conversation-events";

/**
 * 管理工作区会话树。
 *
 * 会话视图固定展示活动工作区、工作区视图平铺全部工作区。
 * 加载态（终端/网页已打开）随持有者心跳变化，因此周期性刷新。
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
