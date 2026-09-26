import { ObjectListPanel } from "../object-list-panel";
import { useI18n } from "../../i18n/use-i18n";
import {
  WEB_SEARCH_PROVIDER_IDS,
  webSearchProviderAvailable,
  webSearchProviderStatus,
  type WebSearchConfig,
  type WebSearchProviderId
} from "./web-search-config";
import { getSearchProvider } from "./search-provider-catalog";

type SearchProviderListProps = {
  config: WebSearchConfig;
  selectedId: WebSearchProviderId;
  onSelect: (id: WebSearchProviderId) => void;
};

/**
 * 渲染 Web 搜索供应商列表及当前配置状态。
 *
 * @param props Web 搜索配置、当前选择和选择回调
 * @returns 可搜索供应商列表
 */
export function SearchProviderList({
  config,
  selectedId,
  onSelect
}: SearchProviderListProps) {
  const { t } = useI18n();
  // 计数与标记按后端实际可路由条件统计，避免展示成已启用却永远不会被调用
  const enabledCount = WEB_SEARCH_PROVIDER_IDS.filter((provider) =>
    webSearchProviderAvailable(config, provider)
  ).length;

  return (
    <ObjectListPanel
      title={t("Search providers", "搜索供应商")}
      items={WEB_SEARCH_PROVIDER_IDS.map((id) => {
        const provider = getSearchProvider(id);
        const Icon = provider.icon;
        const available = webSearchProviderAvailable(config, id);
        return {
          id,
          name: provider.label,
          meta: providerStatusLabel(config, id, available, t),
          icon: <Icon size={14} />,
          marked: available
        };
      })}
      selectedId={selectedId}
      searchPlaceholder={t("Search providers", "搜索供应商")}
      topSlot={(
        <div className="search-provider-summary">
          <span>{t("Enabled", "已启用")}</span>
          <strong>{enabledCount} / {WEB_SEARCH_PROVIDER_IDS.length}</strong>
        </div>
      )}
      onSelect={(id) => onSelect(id as WebSearchProviderId)}
    />
  );
}

/**
 * 生成供应商列表中的状态说明。
 *
 * @param config Web 搜索配置
 * @param provider 供应商标识
 * @param enabled 是否启用
 * @param t 双语文本选择方法
 * @returns 状态说明
 */
function providerStatusLabel(
  config: WebSearchConfig,
  provider: WebSearchProviderId,
  enabled: boolean,
  t: (en: string, zh: string) => string
): string {
  const state = enabled ? t("Enabled", "已启用") : t("Disabled", "已停用");
  const status = webSearchProviderStatus(config, provider);
  const detail = status === "builtin"
    ? t("Built in", "内置")
    : status === "configured"
      ? t("Configured here", "已在此配置")
      : t("Not configured here", "未在此配置");
  return state + " · " + detail;
}
