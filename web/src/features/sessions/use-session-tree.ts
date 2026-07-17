import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";

const EXPANDED_STORAGE_KEY = "sai.session-tree-expanded";

/**
 * 管理工作区会话树、展开状态和运行中会话集合。
 *
 * @returns 会话树查询、展开操作和运行状态
 */
export function useSessionTree() {
  const tree = useQuery({ queryKey: ["session-tree"], queryFn: api.sessions.tree });
  const runs = useQuery({
    queryKey: ["active-runs"],
    queryFn: api.runs.active,
    refetchInterval: 1500
  });
  const [expanded, setExpanded] = useState<Set<string>>(() => {
    try {
      return new Set(JSON.parse(window.localStorage.getItem(EXPANDED_STORAGE_KEY) ?? "[]") as string[]);
    } catch {
      return new Set();
    }
  });

  useEffect(() => {
    const activeId = tree.data?.find((workspace) => workspace.active)?.workspace_id;
    if (!activeId) return;
    // 活动工作区自动展开，并收起其余节点，侧栏只保留一棵打开的树
    setExpanded((current) => {
      if (current.size === 1 && current.has(activeId)) return current;
      return new Set([activeId]);
    });
  }, [tree.data]);

  useEffect(() => {
    window.localStorage.setItem(EXPANDED_STORAGE_KEY, JSON.stringify(Array.from(expanded)));
  }, [expanded]);

  const runningSessions = useMemo(
    () => new Set((runs.data?.runs ?? []).filter((run) => run.status === "running" || run.status === "queued").map((run) => `${run.workspace_id}:${run.session_id}`)),
    [runs.data]
  );

  /** 切换工作区节点的展开状态。 */
  const toggleWorkspace = (workspaceId: string) => {
    setExpanded((current) => {
      // 手风琴：打开一个就关掉其它，再点同一项则收起
      if (current.has(workspaceId) && current.size === 1) return new Set();
      return new Set([workspaceId]);
    });
  };

  return { tree, expanded, runningSessions, toggleWorkspace };
}
