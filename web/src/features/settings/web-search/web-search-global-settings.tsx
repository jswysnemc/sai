import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SettingsPanel, SkNumberInput, SkSelect } from "../kit";
import { WEB_SEARCH_PROVIDER_IDS, webSearchProviderAvailable, type WebSearchConfig } from "./web-search-config";
import { getSearchProvider } from "./search-provider-catalog";

/**
 * 【网页搜索】【默认行为】配置路由、结果数量及请求超时。
 * @param props 搜索配置和更新回调
 * @returns 全局搜索参数面板
 */
export function WebSearchGlobalSettings({ config, onChange }: { config: WebSearchConfig; onChange: (config: WebSearchConfig) => void }) {
  const { t } = useI18n();
  const providerOptions = [
    {
      value: "auto" as const,
      label: t("Automatic", "自动选择"),
      description: t("Try enabled providers in the builtin priority order", "按内置优先级尝试已启用的供应商")
    },
    ...WEB_SEARCH_PROVIDER_IDS
      .filter((provider) => webSearchProviderAvailable(config, provider))
      .map((provider) => ({
        value: provider,
        label: getSearchProvider(provider).label
      }))
  ];

  return <SettingsPanel title={t("Search behavior", "搜索行为")} description={t("Defaults when tool calls omit provider, result count or timeout.", "工具调用未指定供应商、结果数量或超时时使用这些默认值。")}>
    <FieldGrid columns={3}>
      <SettingsField label={t("Default provider", "默认供应商")} anchor="web-search.default_provider" configKey="plugins.web.default_provider" hint={t("Unavailable providers are omitted.", "仅列出当前可用的供应商。")}>
        <SkSelect value={config.default_provider} options={providerOptions} onChange={(value) => onChange({ ...config, default_provider: value })} />
      </SettingsField>
      <SettingsField label={t("Maximum results", "最大结果数量")} anchor="web-search.max_results" configKey="plugins.web.max_results" size="sm">
        <SkNumberInput value={config.max_results} min={1} max={10} integer onChange={(value) => onChange({ ...config, max_results: value ?? 5 })} />
      </SettingsField>
      <SettingsField label={t("Request timeout", "请求超时")} anchor="web-search.timeout_seconds" configKey="plugins.web.timeout_seconds" size="sm">
        <SkNumberInput value={config.timeout_seconds} min={1} max={120} integer unit={t("s", "秒")} onChange={(value) => onChange({ ...config, timeout_seconds: value ?? 20 })} />
      </SettingsField>
    </FieldGrid>
  </SettingsPanel>;
}
