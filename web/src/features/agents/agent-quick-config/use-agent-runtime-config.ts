import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../../api/client";
import type { AgentRuntimeProfilesResponse, UpdateAgentRuntimeRequest } from "../../../api/contracts";

/**
 * 管理输入区 Agent 运行参数的按需读取与保存。
 *
 * @param enabled 快速配置弹层是否已经打开
 * @returns Agent 档案、应用配置和保存状态
 */
export function useAgentRuntimeConfig(enabled: boolean) {
  const queryClient = useQueryClient();
  const profiles = useQuery({
    queryKey: ["agent-runtime-profiles"],
    queryFn: api.agents.runtimeProfiles,
    enabled,
    staleTime: 30_000
  });
  const config = useQuery({
    queryKey: ["config"],
    queryFn: api.config.load,
    enabled,
    staleTime: 30_000
  });
  const update = useMutation({
    mutationFn: ({ agentId, request }: { agentId: string; request: UpdateAgentRuntimeRequest }) => (
      api.agents.updateRuntime(agentId, request)
    ),
    onSuccess: (profile) => {
      queryClient.setQueryData<AgentRuntimeProfilesResponse>(["agent-runtime-profiles"], (current) => ({
        profiles: current?.profiles.map((item) => item.id === profile.id ? profile : item) ?? [profile]
      }));
      void queryClient.invalidateQueries({ queryKey: ["config"] });
    }
  });

  return {
    profiles: profiles.data?.profiles ?? [],
    config: config.data?.config ?? null,
    loading: profiles.isLoading || config.isLoading,
    error: profiles.error ?? config.error ?? update.error,
    save: (agentId: string, request: UpdateAgentRuntimeRequest) => update.mutateAsync({ agentId, request }),
    saving: update.isPending
  };
}
