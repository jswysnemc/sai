import { useEffect, useMemo, useState } from "react";
import type { AppConfig } from "../../../api/contracts";
import { EditorHeader } from "../editor-layout";
import { CliToolConfigEditor } from "./cli-tool-config-editor";
import { CliToolListPanel } from "./cli-tool-list-panel";
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
  const [selectedId, setSelectedId] = useState(toolIds[0] ?? "");

  useEffect(() => {
    if (!toolIds.includes(selectedId)) setSelectedId(toolIds[0] ?? "");
  }, [selectedId, toolIds.join("\u0000")]);

  if (!selectedId) {
    return (
      <section className="settings-editor">
        <EditorHeader
          kicker={t("CLI assistant tools", "CLI 助手工具")}
          title={t("No optional tools found", "没有可选工具")}
          description={t(
            "The current configuration does not contain optional CLI assistant tools.",
            "当前配置中没有 CLI 助手可选工具。"
          )}
        />
      </section>
    );
  }

  const selected = tools.find(({ id }) => id === selectedId) ?? tools[0];
  const entry = getCliToolCatalogEntry(selected.id);

  return (
    <div className="settings-objects-layout cli-tools-layout">
      <CliToolListPanel
        tools={tools}
        selectedId={selected.id}
        onSelect={setSelectedId}
      />
      <section className="settings-editor">
        <EditorHeader
          kicker={t("Optional CLI capability", "CLI 可选能力")}
          title={cliToolLabel(entry, locale)}
          description={cliToolDescription(entry, locale)}
          actions={<code className="cli-tool-id">{selected.id}</code>}
        />
        <CliToolConfigEditor
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
      </section>
    </div>
  );
}
