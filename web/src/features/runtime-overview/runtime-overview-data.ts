import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import type { Subagent, TodoItem, TodoSnapshot } from "../../api/contracts";

type GitLineStats = {
  added: number;
  removed: number;
};

/**
 * 归一化新旧 Todo 接口返回结构。
 *
 * @param data Todo 快照或旧版数组
 * @returns 当前 Todo 项目数组
 */
export function normalizeTodoItems(data: TodoSnapshot | TodoItem[] | undefined): TodoItem[] {
  if (!data) return [];
  return Array.isArray(data) ? data : Array.isArray(data.items) ? data.items : [];
}

/**
 * 统计 Git 补丁中的新增与删除行数。
 *
 * @param patch Git unified diff 文本
 * @returns 新增和删除行数
 */
export function countGitPatchLines(patch: string | undefined): GitLineStats {
  if (!patch) return { added: 0, removed: 0 };
  let added = 0;
  let removed = 0;
  for (const line of patch.split("\n")) {
    if (line.startsWith("+") && !line.startsWith("+++")) added += 1;
    if (line.startsWith("-") && !line.startsWith("---")) removed += 1;
  }
  return { added, removed };
}

/**
 * 读取右上角运行总览所需的 Git、Todo 和子智能体状态。
 *
 * @param sessionId 当前会话标识
 * @returns 三类运行状态及聚合统计
 */
export function useRuntimeOverviewData(sessionId?: string) {
  const git = useQuery({
    queryKey: ["runtime-overview", "git-status"],
    queryFn: () => api.workspace.gitStatus(),
    refetchInterval: 2500,
    retry: false
  });
  const gitSignature = git.data?.entries
    .map((entry) => `${entry.path}:${entry.index_status}:${entry.worktree_status}`)
    .join("|") ?? "";
  const gitDiff = useQuery({
    queryKey: ["runtime-overview", "git-diff", gitSignature],
    queryFn: () => api.workspace.gitReviewDiff("working_tree"),
    enabled: git.data?.status === "ready" && (git.data.entries.length > 0),
    refetchInterval: 5000,
    retry: false
  });
  const todos = useQuery({
    queryKey: ["todos", sessionId],
    queryFn: api.todos.list,
    enabled: Boolean(sessionId),
    refetchInterval: 1500
  });
  const subagents = useQuery({
    queryKey: ["subagents"],
    queryFn: api.subagents.list,
    enabled: Boolean(sessionId),
    refetchInterval: 2000
  });

  const todoItems = normalizeTodoItems(todos.data);
  const subagentItems: Subagent[] = subagents.data ?? [];
  const changedCount = git.data?.entries.length ?? 0;
  const lineStats = countGitPatchLines(gitDiff.data?.patch);

  return {
    git: {
      available: git.data?.status === "ready",
      loading: git.isLoading,
      branch: git.data?.head ?? "",
      changedCount,
      ...lineStats
    },
    todos: {
      items: todoItems,
      completed: todoItems.filter((item) => item.status === "completed").length,
      loading: todos.isLoading
    },
    subagents: {
      items: subagentItems,
      running: subagentItems.filter((item) => item.status === "running").length,
      completed: subagentItems.filter((item) => item.status === "completed").length,
      loading: subagents.isLoading
    }
  };
}
