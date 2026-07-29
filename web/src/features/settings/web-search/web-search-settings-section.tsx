import { useState } from "react";
import type { AppConfig } from "../../../api/contracts";
import { EditorHeader } from "../editor-layout";
import { useI18n } from "../../i18n/use-i18n";
import {
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
  const [selectedProvider, setSelectedProvider] = useState<WebSearchProviderId>("tinyfish");

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
      <section className="settings-editor web-search-overview">
        <EditorHeader
          kicker={t("Web search", "Web 搜索")}
          title={t("Search routing and providers", "搜索路由与供应商")}
          description={t(
            "Configure provider order, credentials, endpoints, and provider-specific retrieval behavior.",
            "配置供应商路由、凭据、服务地址与各供应商的检索行为。"
          )}
          actions={(
            <label className="settings-switch web-search-master-switch">
              <input
                type="checkbox"
                checked={webSearch.enabled}
                aria-label={t("Enable Web search", "启用 Web 搜索")}
                onChange={(event) => updateWebSearch({
                  ...webSearch,
                  enabled: event.target.checked
                })}
              />
              <span aria-hidden="true" />
              <strong>
                {webSearch.enabled ? t("Enabled", "已启用") : t("Disabled", "已停用")}
              </strong>
            </label>
          )}
        />
        <WebSearchGlobalSettings
          config={webSearch}
          onChange={updateWebSearch}
        />
      </section>
      <div className="settings-objects-layout web-search-provider-layout">
        <SearchProviderList
          config={webSearch}
          selectedId={selectedProvider}
          onSelect={setSelectedProvider}
        />
        <SearchProviderEditor
          providerId={selectedProvider}
          config={webSearch}
          secretSentinel={secretSentinel}
          onChange={updateWebSearch}
        />
      </div>
    </div>
  );
}
