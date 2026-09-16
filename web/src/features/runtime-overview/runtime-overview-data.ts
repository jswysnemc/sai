import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";
import { fetchGitDiffStats } from "../../api/git-stats-client";
import type { Subagent, TodoItem, TodoSnapshot } from "../../api/contracts";
import type { Goal } from "../../api/goal-contracts";

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
 * 选出工作概览需要渲染的计划条目。
 *
 * @param items 当前会话的全部计划条目
 * @returns 工作概览中渲染的计划条目
 */
export function selectTodoOverviewItems(items: readonly TodoItem[]): TodoItem[] {
  return [...items];
}

/**
 * 排序工作概览中的子智能体。
 *
 * 1. 运行中任务优先，避免被大量历史终态任务挤出概览
 * 2. 同一状态层级按最近更新时间倒序排列
 *
 * @param items 当前会话的全部子智能体
 * @returns 排序后的完整子智能体列表
 */
export function sortSubagentOverviewItems(items: readonly Subagent[]): Subagent[] {
  return [...items]
    .sort((left, right) => {
      const runningOrder = Number(right.status === "running") - Number(left.status === "running");
      if (runningOrder !== 0) return runningOrder;
      const updatedOrder = right.updated_at - left.updated_at;
      if (updatedOrder !== 0) return updatedOrder;
      return right.started_at - left.started_at;
    });
}

/**
 * 读取右上角运行总览所需的 Git、Todo 和子智能体状态。
 *
 * @param sessionId 当前会话标识
 * @param includeGitStats 当前界面是否需要展示增删行统计
 * @returns 三类运行状态及聚合统计
 */
export function useRuntimeOverviewData(sessionId?: string, includeGitStats = true) {
  const git = useQuery({
    queryKey: ["runtime-overview", "git-status"],
    queryFn: () => api.workspace.gitStatus(),
    refetchInterval: 2500,
    retry: false
  });
  const gitSignature = git.data?.entries
    .map((entry) => `${entry.path}:${entry.index_status}:${entry.worktree_status}`)
    .join("|") ?? "";
  // 1. 【工作概览】【按需统计】仅在展示增删行数时获取轻量统计，折叠的工具栏概览停止查询
  const gitStats = useQuery({
    queryKey: ["runtime-overview", "git-diff-stats", git.data?.repo_root, gitSignature],
    queryFn: () => fetchGitDiffStats(git.data?.repo_root),
    enabled: includeGitStats && git.data?.status === "ready" && (git.data.entries.length > 0),
    refetchInterval: 5000,
    retry: false
  });
  const todos = useQuery({
    queryKey: ["todos", sessionId],
    queryFn: api.todos.list,
    enabled: Boolean(sessionId),
    refetchInterval: 1500
  });
  const goal = useQuery({
    queryKey: ["goal", sessionId],
    queryFn: () => api.goals.read(sessionId!),
    enabled: Boolean(sessionId),
    refetchInterval: (query) => (query.state.data?.goal?.status === "active" ? 2_000 : false)
  });
  const subagents = useQuery({
    queryKey: ["subagents"],
    queryFn: api.subagents.list,
    enabled: Boolean(sessionId),
    refetchInterval: 2000
  });
  // 后台命令启停也要参与活动播报，复用既有查询键，不额外增加请求
  const tasks = useQuery({
    queryKey: ["background-tasks"],
    queryFn: api.backgroundTasks.list,
    enabled: Boolean(sessionId),
    refetchInterval: 3000
  });

  const todoItems = normalizeTodoItems(todos.data);
  const goalItem: Goal | null = goal.data?.goal ?? null;
  const subagentItems: Subagent[] = subagents.data ?? [];
  const changedCount = git.data?.entries.length ?? 0;
  const lineStats = gitStats.data ?? { added: 0, removed: 0 };
  const completedTodos = todoItems.filter((item) => item.status === "completed").length;
  const runningSubagents = subagentItems.filter((item) => item.status === "running").length;
  const runningTasks = tasks.data?.tasks.filter((task) => task.status === "running").length ?? 0;

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
      completed: completedTodos,
      loading: todos.isLoading
    },
    goal: {
      item: goalItem,
      loading: goal.isLoading
    },
    subagents: {
      items: subagentItems,
      overviewItems: sortSubagentOverviewItems(subagentItems),
      running: runningSubagents,
      completed: subagentItems.filter((item) => item.status === "completed").length,
      loading: subagents.isLoading
    },
    tasks: {
      running: runningTasks
    },
    /** 供活动播报比对的运行状态快照 */
    snapshot: {
      runningTasks,
      runningSubagents,
      completedTodos,
      totalTodos: todoItems.length
    }
  };
}
