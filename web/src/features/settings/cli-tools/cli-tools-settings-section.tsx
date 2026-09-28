import { useMemo } from "react";
import { useSettingsItem } from "../shell/use-settings-item";
import type { AppConfig } from "../../../api/contracts";
import { DetailHeader, EmptyGuide } from "../kit";
import { CliToolListPanel } from "./cli-tool-list-panel";
import { CliToolConfigEditor } from "./cli-tool-config-editor";
import {
  cliToolDescription,
  cliToolLabel,
  getCliToolCatalogEntry
} from "./cli-tool-catalog";
import { useI18n } from "../../i18n/use-i18n";
import "./cli-tools-settings.css";

type CliToolsSettingsSectionProps = {
  config: AppConfig;
  secretSentinel: string;
  onConfigChange: (config: AppConfig) => void;
};

/**
 * 渲染 CLI 助手可选工具设置页，Web 搜索由独立页面管理。
 *
 * @param props 应用配置、敏感字段占位符和更新回调
 * @returns CLI 助手工具列表与配置编辑区
 */
export function CliToolsSettingsSection({
  config,
  secretSentinel,
  onConfigChange
}: CliToolsSettingsSectionProps) {
  const { locale, t } = useI18n();
  const tools = useMemo(
    () => Object.entries(config.plugins ?? {})
      .filter(([id]) => id !== "web")
      .map(([id, toolConfig]) => ({ id, config: toolConfig })),
    [config.plugins]
  );
  const toolIds = tools.map(({ id }) => id);
  const [selectedId, setSelectedId] = useSettingsItem(toolIds);

  if (!selectedId) return <EmptyGuide title={t("No optional tools found", "没有可选工具")} description={t("No optional CLI tools are present in the current configuration.", "当前配置中没有 CLI 助手可选工具。")} />;

  const selected = tools.find(({ id }) => id === selectedId) ?? tools[0];
  const entry = getCliToolCatalogEntry(selected.id);

  return (
    <div className="min-w-0">
        <CliToolListPanel tools={tools} selectedId={selected.id} onSelect={setSelectedId} />
        <DetailHeader
          title={cliToolLabel(entry, locale)}
          subtitle={cliToolDescription(entry, locale)}
        />
        <CliToolConfigEditor
          toolId={selected.id}
          config={selected.config}
          secretSentinel={secretSentinel}
          onChange={(next) => onConfigChange({
            ...config,
            plugins: {
              ...(config.plugins ?? {}),
              [selected.id]: next
            }
          })}
        />
    </div>
  );
}
