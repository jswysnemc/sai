import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import type { AppConfig, ProviderConfig } from "../../api/contracts";
import { api } from "../../api/client";
import type { GatewayId, SettingsConfigController } from "./settings-types";

/**
 * 管理设置配置的读取、结构化草稿、JSON 同步和保存。
 *
 * 规则：
 * 1. serverConfig 仅来自加载/保存成功响应
 * 2. draftConfig 是编辑中的结构化对象
 * 3. raw 与 draft 尽量同步；非法 JSON 只更新 raw 并暴露 parseError，不污染 draft
 *
 * @returns 设置页配置控制器
 */
export function useSettingsConfig(): SettingsConfigController {
  const queryClient = useQueryClient();
  const response = useQuery({ queryKey: ["config"], queryFn: api.config.load });
  const [serverConfig, setServerConfig] = useState<AppConfig | null>(null);
  const [draftConfig, setDraftConfig] = useState<AppConfig | null>(null);
  const [raw, setRaw] = useState("");
  const [rawParseError, setRawParseError] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    if (!response.data || dirty) return;
    // 1. 仅在无本地草稿时用服务端快照重置
    setServerConfig(response.data.config);
    setDraftConfig(response.data.config);
    setRaw(JSON.stringify(response.data.config, null, 2));
    setRawParseError(null);
  }, [response.data, dirty]);

  const save = useMutation({
    mutationFn: async () => {
      // 1. 优先保存合法 raw；否则回退 draft
      const payload = resolveSavePayload(raw, draftConfig, rawParseError);
      return api.config.save(payload);
    },
    onSuccess: async (saved) => {
      setServerConfig(saved.config);
      setDraftConfig(saved.config);
      setRaw(JSON.stringify(saved.config, null, 2));
      setRawParseError(null);
      setDirty(false);
      queryClient.setQueryData(["config"], saved);
      await queryClient.invalidateQueries({ queryKey: ["gateways"] });
    }
  });

  /**
   * 更新完整结构化配置并同步 JSON 文本。
   *
   * @param updated 新应用配置
   */
  const updateConfig = (updated: AppConfig) => {
    setDraftConfig(updated);
    setRaw(JSON.stringify(updated, null, 2));
    setRawParseError(null);
    setDirty(true);
    save.reset();
  };

  /**
   * 更新高级 JSON 文本；仅在解析成功时写回 draft。
   *
   * @param value 新 JSON 文本
   */
  const updateRaw = (value: string) => {
    setRaw(value);
    setDirty(true);
    save.reset();
    try {
      const parsed = JSON.parse(value) as AppConfig;
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        throw new Error("AppConfig must be a JSON object");
      }
      setDraftConfig(parsed);
      setRawParseError(null);
    } catch (error) {
      // 2. 非法 JSON 不覆盖 draft，保留上一份结构化状态
      setRawParseError(error instanceof Error ? error.message : String(error));
    }
  };

  /**
   * 更新指定供应商配置。
   *
   * @param index 供应商索引
   * @param patch 供应商字段补丁
   */
  const updateProvider = (index: number, patch: Partial<ProviderConfig>) => {
    if (!draftConfig) return;
    const previousId = draftConfig.providers[index]?.id;
    const providers = draftConfig.providers.map((provider, providerIndex) => (
      providerIndex === index ? { ...provider, ...patch } : provider
    ));
    const activeProvider = patch.id && previousId === draftConfig.active_provider ? patch.id : draftConfig.active_provider;
    updateConfig({ ...draftConfig, active_provider: activeProvider, providers });
  };

  /**
   * 更新指定网关配置。
   *
   * @param gateway 网关标识
   * @param patch 网关字段补丁
   */
  const updateGateway = (gateway: GatewayId, patch: Record<string, unknown>) => {
    if (!draftConfig) return;
    updateConfig({
      ...draftConfig,
      gateways: {
        ...draftConfig.gateways,
        [gateway]: { ...draftConfig.gateways[gateway], ...patch }
      }
    });
  };

  /** 保存当前配置并等待服务端校验完成。 */
  const saveConfig = async () => {
    await save.mutateAsync();
  };

  const combinedError = useMemo(() => {
    if (rawParseError) return new Error(rawParseError);
    return (response.error ?? save.error) as Error | null;
  }, [rawParseError, response.error, save.error]);

  return {
    config: draftConfig,
    raw,
    dirty,
    loading: response.isLoading,
    saving: save.isPending,
    error: combinedError,
    saved: save.isSuccess,
    updateConfig,
    updateRaw,
    updateProvider,
    updateGateway,
    saveConfig
  };
}

/**
 * 决定提交给服务端的配置对象。
 *
 * @param raw JSON 文本
 * @param draft 结构化草稿
 * @param rawParseError raw 解析错误
 * @returns 可保存的 AppConfig
 */
function resolveSavePayload(
  raw: string,
  draft: AppConfig | null,
  rawParseError: string | null
): AppConfig {
  if (!rawParseError) {
    try {
      const parsed = JSON.parse(raw) as AppConfig;
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
        return parsed;
      }
    } catch {
      // fall through
    }
  }
  if (draft) return draft;
  throw new Error("Configuration is not ready to save");
}
