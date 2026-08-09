import { useQuery } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../../api/client";
import { isSideConversationSessionTitle } from "../side-conversation/side-conversation-events";

/**
 * 管理工作区会话树与运行中会话集合。
 *
 * 会话视图固定展示活动工作区、工作区视图平铺全部工作区，
 * 早先的手风琴展开状态没有消费方了，随之移除。
 *
 * @returns 会话树查询与运行中会话集合
 */
export function useSessionTree() {
  const tree = useQuery({
    queryKey: ["session-tree"],
    queryFn: api.sessions.tree,
    select: (workspaces) => workspaces.map((workspace) => ({
      ...workspace,
      sessions: workspace.sessions.filter((session) => !isSideConversationSessionTitle(session.title))
    }))
  });
  const runs = useQuery({
    queryKey: ["active-runs"],
    queryFn: api.runs.active,
    refetchInterval: 1500
  });

  const runningSessions = useMemo(
    () => new Set((runs.data?.runs ?? []).filter((run) => run.status === "running" || run.status === "queued").map((run) => `${run.workspace_id}:${run.session_id}`)),
    [runs.data]
  );

  return { tree, runningSessions };
}
