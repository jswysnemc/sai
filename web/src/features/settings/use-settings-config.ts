import type { AppConfig, ProviderConfig } from "../../api/contracts";
import { api } from "../../api/client";
import { renameNewSessionProviderReference } from "../sessions/new-session-preferences";
import { useConfigDocument } from "./use-config-document";
import type { GatewayId, SettingsConfigController } from "./settings-types";

/**
 * 管理全局 AppConfig 的读取、结构化草稿和保存。
 *
 * 文档状态机由 useConfigDocument 承载；本 Hook 补充供应商与网关的
 * 领域更新方法。高级 JSON 文本由 advanced 分区自持，保存路径因此
 * 只有结构化草稿一个真相源。
 *
 * @returns 设置页配置控制器
 */
export function useSettingsConfig(): SettingsConfigController {
  const document = useConfigDocument({
    queryKey: ["config"] as const,
    load: api.config.load,
    extract: (response) => response.config,
    save: (config: AppConfig) => api.config.save(config),
    onSaved: async (_, queryClient) => {
      // 内核查询在设置页通常未挂载；直接移除旧值，避免返回聊天页时闪现旧模型
      queryClient.removeQueries({ queryKey: ["engine-status"] });
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["gateways"] }),
        queryClient.invalidateQueries({ queryKey: ["system-usage"] })
      ]);
    }
  });

  /**
   * 【设置】【供应商更新】更新指定供应商配置并同步关联引用。
   *
   * @param index 供应商索引
   * @param patch 供应商字段补丁
   * @returns 无返回值
   */
  const updateProvider = (index: number, patch: Partial<ProviderConfig>) => {
    const config = document.draft;
    if (!config) return;
    const previousId = config.providers[index]?.id;
    const providers = config.providers.map((provider, providerIndex) => (
      providerIndex === index ? { ...provider, ...patch } : provider
    ));
    const activeProvider = patch.id && previousId === config.active_provider ? patch.id : config.active_provider;
    const session = patch.id
      ? renameNewSessionProviderReference(config.session, previousId, patch.id)
      : config.session;
    document.update({ ...config, active_provider: activeProvider, providers, session });
  };

  /**
   * 更新指定网关配置。
   *
   * @param gateway 网关标识
   * @param patch 网关字段补丁
   */
  const updateGateway = (gateway: GatewayId, patch: Record<string, unknown>) => {
    const config = document.draft;
    if (!config) return;
    document.update({
      ...config,
      gateways: {
        ...config.gateways,
        [gateway]: { ...config.gateways[gateway], ...patch }
      }
    });
  };

  return {
    config: document.draft,
    secretSentinel: document.response.data?.secret_sentinel ?? "",
    dirty: document.dirty,
    loading: document.loading,
    saving: document.saving,
    error: document.loadError ?? document.saveError,
    saved: document.saved,
    updateConfig: document.update,
    updateProvider,
    updateGateway,
    saveConfig: async () => {
      await document.saveNow();
    },
    retry: document.retry
  };
}
