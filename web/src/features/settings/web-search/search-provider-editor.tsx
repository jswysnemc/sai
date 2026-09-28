import { useI18n } from "../../i18n/use-i18n";
import { DetailHeader, InlineSwitch, StatusBadge } from "../kit";
import { normalizeWebSearchSelection, webSearchProviderEnabled, webSearchProviderStatus, type WebSearchConfig, type WebSearchProviderId } from "./web-search-config";
import { getSearchProvider, searchProviderDescription } from "./search-provider-catalog";
import { SearchProviderFields } from "./search-provider-fields";

/**
 * 【网页搜索】【供应商详情】显示接入状态，更新供应商字段并校正默认路由。
 * @param props 供应商标识、搜索配置、脱敏标记及更新回调
 * @returns 供应商详情
 */
export function SearchProviderEditor({ providerId, config, secretSentinel, onChange }: {
  providerId: WebSearchProviderId;
  config: WebSearchConfig;
  secretSentinel: string;
  onChange: (config: WebSearchConfig) => void;
}) {
  const { locale, t } = useI18n();
  const provider = getSearchProvider(providerId);
  const enabled = webSearchProviderEnabled(config, providerId);
  const status = webSearchProviderStatus(config, providerId);

  /**
   * 合并字段并校正不可用的默认供应商。
   * @param patch 供应商字段补丁
   * @returns 无返回值
   */
  const update = (patch: Partial<WebSearchConfig>) => onChange(normalizeWebSearchSelection({ ...config, ...patch }));

  return <>
    <DetailHeader title={provider.label} subtitle={searchProviderDescription(provider, locale)} badges={<StatusBadge tone={status === "missing" ? "warning" : "success"}>{status === "builtin" ? t("Built in", "内置") : status === "configured" ? t("Configured", "已配置") : t("Not configured here", "未在此配置")}</StatusBadge>}
      actions={<InlineSwitch checked={enabled} label={enabled ? t("Enabled", "已启用") : t("Disabled", "已停用")} onChange={(checked) => update({ [`${providerId}_enabled`]: checked })} />} />
    <SearchProviderFields providerId={providerId} config={config} secretSentinel={secretSentinel} update={update} />
  </>;
}
