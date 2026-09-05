import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../../api/client";
import { loadSubagentModels, saveSubagentModels, type SubagentModelChoice } from "../../../api/subagent-models";

/**
 * 【Web】【子任务模型】管理子任务共享设置与类型覆盖的读取、保存和缓存。
 *
 * @param enabled 快速配置弹层是否已经打开
 * @returns 子任务设置、应用配置和保存状态
 */
export function useSubagentModelSettings(enabled: boolean) {
  const queryClient = useQueryClient();
  const settings = useQuery({
    queryKey: ["subagent-model-settings"],
    queryFn: loadSubagentModels,
    enabled,
    staleTime: 0
  });
  const config = useQuery({
    queryKey: ["config"],
    queryFn: api.config.load,
    enabled,
    staleTime: 0
  });
  const update = useMutation({
    mutationFn: ({ profileId, selection }: { profileId: string | null; selection: SubagentModelChoice }) => (
      saveSubagentModels(profileId, selection)
    ),
    onSuccess: (saved) => {
      queryClient.setQueryData(["subagent-model-settings"], saved);
      void queryClient.invalidateQueries({ queryKey: ["config"] });
    }
  });

  return {
    settings: settings.data ?? null,
    config: config.data?.config ?? null,
    loading: settings.isLoading || config.isLoading,
    error: settings.error ?? config.error ?? update.error,
    save: (profileId: string | null, selection: SubagentModelChoice) => update.mutateAsync({ profileId, selection }),
    clearError: update.reset,
    saving: update.isPending
  };
}
