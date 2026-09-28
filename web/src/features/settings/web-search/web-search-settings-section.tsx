import type { AppConfig } from "../../../api/contracts";
import { MasterDetail, SwitchField } from "../kit";
import { useSettingsItem } from "../shell/use-settings-item";
import { useI18n } from "../../i18n/use-i18n";
import {
  WEB_SEARCH_PROVIDER_IDS,
  normalizeWebSearchSelection,
  readWebSearchConfig,
  writeWebSearchConfig,
  type WebSearchConfig,
  type WebSearchProviderId
} from "./web-search-config";
import { WebSearchGlobalSettings } from "./web-search-global-settings";
import { SearchProviderList } from "./search-provider-list";
import { SearchProviderEditor } from "./search-provider-editor";
import "./web-search-settings.css";

type WebSearchSettingsSectionProps = {
  config: AppConfig;
  secretSentinel: string;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染独立 Web 搜索设置页。
 *
 * @param props 应用配置、敏感字段占位符和更新回调
 * @returns Web 搜索全局行为与供应商详细配置
 */
export function WebSearchSettingsSection({
  config,
  secretSentinel,
  onConfigChange
}: WebSearchSettingsSectionProps) {
  const { t } = useI18n();
  const webSearch = normalizeWebSearchSelection(readWebSearchConfig(config));
  const [selectedId, setSelectedProvider] = useSettingsItem(WEB_SEARCH_PROVIDER_IDS);
  const selectedProvider = selectedId as WebSearchProviderId;

  /**
   * 将 Web 搜索配置写回历史兼容的 plugins.web 键。
   *
   * @param next 新 Web 搜索配置
   * @returns 无返回值
   */
  const updateWebSearch = (next: WebSearchConfig): void => {
    onConfigChange(writeWebSearchConfig(config, normalizeWebSearchSelection(next)));
  };

  return (
    <div className={webSearch.enabled ? "web-search-settings" : "web-search-settings is-disabled"}>
      <SwitchField label={t("Enable Web search", "启用 Web 搜索")} anchor="web-search.enabled" configKey="plugins.web.enabled" checked={webSearch.enabled} onChange={(enabled) => updateWebSearch({ ...webSearch, enabled })} />
        <WebSearchGlobalSettings
          config={webSearch}
          onChange={updateWebSearch}
        />
      <MasterDetail list={<SearchProviderList
          config={webSearch}
          selectedId={selectedProvider}
          onSelect={setSelectedProvider}
        />}>
        <SearchProviderEditor
          providerId={selectedProvider}
          config={webSearch}
          secretSentinel={secretSentinel}
          onChange={updateWebSearch}
        />
      </MasterDetail>
    </div>
  );
}
