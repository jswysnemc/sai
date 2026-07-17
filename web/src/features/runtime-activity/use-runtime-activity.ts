import { useQuery } from "@tanstack/react-query";
import { api } from "../../api/client";

export type RuntimeActivity = {
  runningSubagents: number;
  runningTasks: number;
  total: number;
  active: boolean;
};

/**
 * 聚合运行中的子智能体与后台任务数量,供全局活动指示器使用。
 *
 * 复用既有查询键,与各面板共享同一轮询结果,不额外增加请求。
 *
 * @returns 运行时活动摘要
 */
export function useRuntimeActivity(): RuntimeActivity {
  const subagents = useQuery({ queryKey: ["subagents"], queryFn: api.subagents.list, refetchInterval: 2000 });
  const tasks = useQuery({ queryKey: ["background-tasks"], queryFn: api.backgroundTasks.list, refetchInterval: 3000 });
  const runningSubagents = subagents.data?.filter((subagent) => subagent.status === "running").length ?? 0;
  const runningTasks = tasks.data?.tasks.filter((task) => task.status === "running").length ?? 0;
  const total = runningSubagents + runningTasks;
  return { runningSubagents, runningTasks, total, active: total > 0 };
}
