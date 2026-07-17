import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../../api/client";
import type { AppConfig } from "../../../api/contracts";

export type AgentConfigDraft = {
  webAgent: string;
  tuiAgent: string;
  cliAgent: string;
  gatewayAgent: string;
};

/**
 * 从应用配置读取 agent 相关字段草稿。
 *
 * @param config 应用配置
 * @returns agent 配置草稿
 */
export function readAgentConfigDraft(config: AppConfig): AgentConfigDraft {
  return {
    webAgent: config.default_agent ?? "general",
    tuiAgent: config.tui_agent ?? "general",
    cliAgent: config.cli_agent ?? "default",
    gatewayAgent: config.gateway_agent ?? "gateway"
  };
}

/**
 * 管理 agent 配置弹窗的读取与保存。
 *
 * 复用 config 查询键,保存后失效缓存,让主界面 agent 选择器同步刷新。
 *
 * @returns 配置、加载状态与保存方法
 */
export function useAgentConfig() {
  const queryClient = useQueryClient();
  const query = useQuery({ queryKey: ["config"], queryFn: api.config.load });
  const save = useMutation({
    mutationFn: (draft: AgentConfigDraft) => {
      const current = query.data?.config;
      if (!current) throw new Error("配置尚未加载");
      const next: AppConfig = {
        ...current,
        default_agent: draft.webAgent === "default" ? null : draft.webAgent,
        tui_agent: draft.tuiAgent === "default" ? null : draft.tuiAgent,
        cli_agent: draft.cliAgent === "default" ? null : draft.cliAgent,
        gateway_agent: draft.gatewayAgent === "default" ? null : draft.gatewayAgent
      };
      return api.config.save(next as unknown as Record<string, unknown>);
    },
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["config"] })
  });
  return {
    config: query.data?.config ?? null,
    isLoading: query.isLoading,
    error: query.error,
    save: save.mutateAsync,
    saving: save.isPending,
    saveError: save.error
  };
}
