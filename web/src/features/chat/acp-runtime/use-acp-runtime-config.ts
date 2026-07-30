import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../../api/client";
import type { AcpOptionValue } from "./acp-runtime-options";

type UseAcpRuntimeConfigResult = {
  /** 当前已保存的 ACP 配置片段 */
  acp: Record<string, unknown>;
  /** 配置是否仍在读取 */
  loading: boolean;
  /** 读取或保存失败的原因 */
  error: unknown;
  /** 写入标准类别字段 */
  saveField: (key: string, value: AcpOptionValue) => void;
  /** 写入 agent 自定义配置项 */
  saveOption: (id: string, value: AcpOptionValue) => void;
  /** 是否正在保存 */
  saving: boolean;
};

/**
 * 管理主页面对 ACP 运行参数的按需读取与即时保存。
 *
 * ACP 的权限模式与自定义配置项没有会话级覆盖通道，只能写入全局配置，
 * 因此这里沿用 Agent 快速配置的即时保存范式：改一项即落盘并刷新查询。
 *
 * @param enabled 弹层是否已经打开
 * @returns ACP 配置片段与写入入口
 */
export function useAcpRuntimeConfig(enabled: boolean): UseAcpRuntimeConfigResult {
  const queryClient = useQueryClient();
  const config = useQuery({
    queryKey: ["config"],
    queryFn: api.config.load,
    enabled,
    staleTime: 30_000
  });
  const acp = readAcpSection(config.data?.config);

  const save = useMutation({
    mutationFn: (patch: Record<string, unknown>) => {
      const current = (config.data?.config ?? {}) as Record<string, unknown>;
      const agent = (current.agent as Record<string, unknown> | undefined) ?? {};
      return api.config.save({
        ...current,
        agent: { ...agent, acp: { ...acp, ...patch } }
      });
    },
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["config"] });
      // ACP 能力快照随配置变化，连带刷新内核状态
      void queryClient.invalidateQueries({ queryKey: ["engine-status"] });
    }
  });

  return {
    acp,
    loading: config.isLoading,
    error: config.error ?? save.error,
    saveField: (key, value) => save.mutate({ [key]: value }),
    saveOption: (id, value) => {
      const options = (acp.config_options as Record<string, unknown> | undefined) ?? {};
      save.mutate({ config_options: { ...options, [id]: value } });
    },
    saving: save.isPending
  };
}

/**
 * 从整份配置中读取 ACP 片段。
 *
 * @param config 应用配置
 * @returns ACP 配置片段；缺失时为空对象
 */
function readAcpSection(config: unknown): Record<string, unknown> {
  if (!config || typeof config !== "object") return {};
  const agent = (config as Record<string, unknown>).agent;
  if (!agent || typeof agent !== "object") return {};
  const acp = (agent as Record<string, unknown>).acp;
  return acp && typeof acp === "object" ? (acp as Record<string, unknown>) : {};
}
