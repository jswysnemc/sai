import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";
import { api } from "../../api/client";
import type { BackgroundTask } from "../../api/contracts";
import { filterBackgroundTasks, isBackgroundTaskRunning, sortBackgroundTasks, type BackgroundTaskFilter } from "./background-task-utils";

const TASKS_KEY = ["background-tasks"] as const;
const OUTPUT_KEY = ["background-task-output"] as const;

/**
 * 【后台任务】【状态同步】管理任务轮询、输出读取以及停止和清理后的缓存同步。
 * @param selectedId 用户选择的任务标识，缺失或失效时优先选择运行中的任务
 * @param filter 当前列表筛选范围
 * @returns 排序后的任务、当前详情、查询状态和管理操作
 */
export function useBackgroundTasks(selectedId: string | null, filter: BackgroundTaskFilter = "all") {
  const queryClient = useQueryClient();
  const list = useQuery({ queryKey: TASKS_KEY, queryFn: api.backgroundTasks.list, refetchInterval: 3000 });
  const tasks = useMemo(() => sortBackgroundTasks(list.data?.tasks ?? []), [list.data]);
  const visibleTasks = useMemo(() => filterBackgroundTasks(tasks, filter), [tasks, filter]);
  const selected = visibleTasks.find((task) => task.id === selectedId) ?? visibleTasks[0];
  const output = useQuery({
    // 1. 【后台任务】【终态输出】状态变化后重新读取尾部，避免停止轮询时遗漏最后一段输出
    queryKey: [...OUTPUT_KEY, selected?.id, selected?.status],
    queryFn: () => api.backgroundTasks.output(selected!.id, 2000),
    enabled: Boolean(selected),
    refetchInterval: selected && isBackgroundTaskRunning(selected) ? 3000 : false
  });
  const stop = useMutation({
    mutationFn: (request: string | { id: string; force: boolean }) => typeof request === "string"
      ? api.backgroundTasks.stop(request) : api.backgroundTasks.stop(request.id, request.force),
    onSuccess: async ({ task }) => {
      // 2. 【后台任务】【停止同步】取消旧列表请求，防止较晚返回的运行状态覆盖停止结果
      await queryClient.cancelQueries({ queryKey: TASKS_KEY });
      queryClient.setQueryData<{ tasks: BackgroundTask[] }>(TASKS_KEY, (current) => current && ({
        ...current, tasks: current.tasks.map((item) => item.id === task.id ? task : item)
      }));
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: TASKS_KEY }),
        queryClient.invalidateQueries({ queryKey: [...OUTPUT_KEY, task.id] })
      ]);
    }
  });
  const cleanup = useMutation({
    mutationFn: () => api.backgroundTasks.cleanup(false),
    onSuccess: async ({ removed }) => {
      // 3. 【后台任务】【清理同步】移除记录前取消相关输出请求，避免继续读取已清理的任务
      await queryClient.cancelQueries({ queryKey: TASKS_KEY });
      await Promise.all(removed.map((id) => queryClient.cancelQueries({ queryKey: [...OUTPUT_KEY, id] })));
      queryClient.setQueryData<{ tasks: BackgroundTask[] }>(TASKS_KEY, (current) => current && ({
        ...current, tasks: current.tasks.filter((task) => !removed.includes(task.id))
      }));
      removed.forEach((id) => queryClient.removeQueries({ queryKey: [...OUTPUT_KEY, id] }));
      await queryClient.invalidateQueries({ queryKey: TASKS_KEY });
    }
  });

  /**
   * 【后台任务】【手动刷新】刷新列表及仍然存在的选中任务输出。
   * @returns 查询完成后结束，失败时抛出接口错误
   */
  const refresh = async () => {
    const latest = await list.refetch({ throwOnError: true });
    if (selected && latest.data?.tasks.some((task) => task.id === selected.id)) {
      await queryClient.invalidateQueries({ queryKey: [...OUTPUT_KEY, selected.id] }, { throwOnError: true });
    }
  };

  return {
    tasks, visibleTasks, selected, output: output.data, loading: list.isLoading, error: list.error,
    outputLoading: output.isLoading, outputError: output.error,
    refreshing: list.isFetching || output.isFetching,
    stoppingId: stop.isPending ? (typeof stop.variables === "string" ? stop.variables : stop.variables.id) : null, cleaning: cleanup.isPending,
    refresh, stop: stop.mutateAsync, cleanup: cleanup.mutateAsync
  };
}
