import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";

/** 管理后台任务列表轮询、输出读取、停止和清理操作。 */
export function useBackgroundTasks(selectedId: string | null) {
  const queryClient = useQueryClient();
  const tasks = useQuery({ queryKey: ["background-tasks"], queryFn: api.backgroundTasks.list, refetchInterval: 3000 });
  const output = useQuery({
    queryKey: ["background-task-output", selectedId],
    queryFn: () => api.backgroundTasks.output(selectedId!),
    enabled: Boolean(selectedId),
    refetchInterval: selectedId ? 3000 : false
  });
  const refresh = () => Promise.all([tasks.refetch(), selectedId ? output.refetch() : Promise.resolve()]);
  const stop = useMutation({
    mutationFn: api.backgroundTasks.stop,
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["background-tasks"] })
  });
  const cleanup = useMutation({
    mutationFn: () => api.backgroundTasks.cleanup(false),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["background-tasks"] })
  });
  return { tasks: tasks.data?.tasks ?? [], output: output.data, loading: tasks.isLoading, error: tasks.error ?? output.error, refresh, stop: stop.mutateAsync, cleanup: cleanup.mutateAsync };
}
