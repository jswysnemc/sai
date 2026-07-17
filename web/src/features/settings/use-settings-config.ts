import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import type { AppConfig, ProviderConfig } from "../../api/contracts";
import { api } from "../../api/client";
import type { GatewayId, SettingsConfigController } from "./settings-types";

/**
 * 管理设置配置的读取、结构化修改、JSON 同步和保存。
 *
 * @returns 设置页配置控制器
 */
export function useSettingsConfig(): SettingsConfigController {
  const queryClient = useQueryClient();
  const response = useQuery({ queryKey: ["config"], queryFn: api.config.load });
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [raw, setRaw] = useState("");
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    if (!response.data || dirty) return;
    setConfig(response.data.config);
    setRaw(JSON.stringify(response.data.config, null, 2));
  }, [response.data]);

  const save = useMutation({
    mutationFn: async () => api.config.save(JSON.parse(raw) as AppConfig),
    onSuccess: async (saved) => {
      setConfig(saved.config);
      setRaw(JSON.stringify(saved.config, null, 2));
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
    setConfig(updated);
    setRaw(JSON.stringify(updated, null, 2));
    setDirty(true);
    save.reset();
  };

  /**
   * 更新高级 JSON 文本。
   *
   * @param value 新 JSON 文本
   */
  const updateRaw = (value: string) => {
    setRaw(value);
    setDirty(true);
    save.reset();
  };

  /**
   * 更新指定供应商配置。
   *
   * @param index 供应商索引
   * @param patch 供应商字段补丁
   */
  const updateProvider = (index: number, patch: Partial<ProviderConfig>) => {
    if (!config) return;
    const previousId = config.providers[index]?.id;
    const providers = config.providers.map((provider, providerIndex) => (
      providerIndex === index ? { ...provider, ...patch } : provider
    ));
    const activeProvider = patch.id && previousId === config.active_provider ? patch.id : config.active_provider;
    updateConfig({ ...config, active_provider: activeProvider, providers });
  };

  /**
   * 更新指定网关配置。
   *
   * @param gateway 网关标识
   * @param patch 网关字段补丁
   */
  const updateGateway = (gateway: GatewayId, patch: Record<string, unknown>) => {
    if (!config) return;
    updateConfig({
      ...config,
      gateways: {
        ...config.gateways,
        [gateway]: { ...config.gateways[gateway], ...patch }
      }
    });
  };

  /** 保存当前 JSON 配置并等待服务端校验完成。 */
  const saveConfig = async () => {
    await save.mutateAsync();
  };

  return {
    config,
    raw,
    dirty,
    loading: response.isLoading,
    saving: save.isPending,
    error: (response.error ?? save.error) as Error | null,
    saved: save.isSuccess,
    updateConfig,
    updateRaw,
    updateProvider,
    updateGateway,
    saveConfig
  };
}
