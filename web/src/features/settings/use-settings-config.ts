import type { AppConfig, ProviderConfig } from "../../api/contracts";
import { useState } from "react";
import { parseSettingsJson } from "./advanced/settings-json";
import { api } from "../../api/client";
import { renameNewSessionProviderReference } from "../sessions/new-session-preferences";
import { useConfigDocument } from "./use-config-document";
import type { GatewayId, SettingsConfigController } from "./settings-types";

/**
 * 管理全局 AppConfig 的读取、结构化草稿和保存。
 *
 * 文档状态机由 useConfigDocument 承载；本 Hook 补充供应商与网关的
 * 领域更新方法。高级 JSON 文本由控制器持有，非法文本跨分区保留，
 * 只有通过解析的完整配置才允许保存。
 *
 * @returns 设置页配置控制器
 */
export function useSettingsConfig(): SettingsConfigController {
  const [jsonDraft, setJsonDraft] = useState<{ text: string; error: string | null } | null>(null);
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
   * 【设置】【结构化编辑】更新配置并同步完整文档的文本表示。
   * @param config 编辑后的配置
   * @returns 无返回值
   */
  const updateConfig = (config: AppConfig) => {
    setJsonDraft(null);
    document.update(config);
  };

  /**
   * 【设置】【JSON 编辑】保留原始文本；合法时同步配置，非法时仅标记待保存。
   * @param text 编辑器中的完整文本
   * @returns 无返回值
   */
  const updateJson = (text: string) => {
    const parsed = parseSettingsJson(text);
    setJsonDraft({ text, error: parsed.error });
    if (parsed.config) document.update(parsed.config);
    else document.markDirty();
  };

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
    updateConfig({ ...config, active_provider: activeProvider, providers, session });
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
    updateConfig({
      ...config,
      gateways: {
        ...config.gateways,
        [gateway]: { ...config.gateways[gateway], ...patch }
      }
    });
  };

  return {
    config: document.draft,
    baseline: document.baseline,
    secretSentinel: document.response.data?.secret_sentinel ?? "",
    dirty: document.dirty,
    loading: document.loading,
    saving: document.saving,
    error: document.loadError ?? document.saveError,
    saveError: document.saveError,
    saved: document.saved,
    rawJson: jsonDraft?.text ?? JSON.stringify(document.draft, null, 2),
    jsonError: jsonDraft?.error ?? null,
    updateJson,
    updateConfig,
    updateProvider,
    updateGateway,
    saveConfig: async () => {
      if (jsonDraft?.error) throw new Error(jsonDraft.error);
      await document.saveNow();
      setJsonDraft(null);
    },
    discard: () => { setJsonDraft(null); document.discard(); },
    retry: document.retry
  };
}
