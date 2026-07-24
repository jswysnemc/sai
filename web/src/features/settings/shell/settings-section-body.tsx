import { AdvancedSettingsSection } from "../advanced-settings-section";
import { AgentSettingsSection } from "../agents/agent-settings-section";
import { AppearanceSettingsSection } from "../appearance-settings-section";
import { GatewaySettingsSection } from "../gateway-settings-section";
import { GitSettingsPanel } from "../git/git-settings-panel";
import { ProviderSettingsSection } from "../provider-settings-section";
import { PluginSettingsSection } from "../plugin-settings-section";
import { RuntimeSettingsSection } from "../runtime-settings-section";
import { MemorySettingsSection } from "../memory-settings-section";
import { HooksSettingsSection } from "../hooks-settings-section";
import { McpSettingsSection } from "../mcp-settings-section";
import { SkillsSettingsSection } from "../skills/skills-settings-section";
import { UsageStatsSection } from "../usage-stats-section";
import type { SettingsConfigController, SettingsSectionId } from "../settings-types";
import type { ThemeId } from "../../theme/theme";
import { useI18n } from "../../i18n/use-i18n";

type SettingsSectionBodyProps = {
  section: SettingsSectionId;
  settings: SettingsConfigController;
  theme: ThemeId;
  onThemeChange: (theme: ThemeId) => void;
};

/**
 * 按 section id 挂载对应设置面板。
 *
 * @param props 当前 section、全局配置控制器与外观偏好
 * @returns section 内容
 */
export function SettingsSectionBody({
  section,
  settings,
  theme,
  onThemeChange
}: SettingsSectionBodyProps) {
  const { t } = useI18n();
  const needsConfig = sectionNeedsAppConfig(section);

  // 1. 依赖 AppConfig 的 section：加载中 / 缺失时给出状态
  if (needsConfig && settings.loading) {
    return <div className="settings-state">{t("Loading configuration", "正在读取配置")}</div>;
  }
  if (needsConfig && !settings.config) {
    return <div className="settings-state">{t("Configuration unavailable", "配置不可用")}</div>;
  }

  // 2. 按 id 渲染；独立面不要求 config
  switch (section) {
    case "providers":
      return (
        <ProviderSettingsSection
          config={settings.config!}
          onConfigChange={settings.updateConfig}
          onProviderChange={settings.updateProvider}
        />
      );
    case "agents":
      return (
        <AgentSettingsSection
          config={settings.config!}
          onConfigChange={settings.updateConfig}
        />
      );
    case "plugins":
      return (
        <PluginSettingsSection
          config={settings.config!}
          onConfigChange={settings.updateConfig}
        />
      );
    case "runtime":
      return (
        <RuntimeSettingsSection
          config={settings.config!}
          onConfigChange={settings.updateConfig}
        />
      );
    case "skills":
      return (
        <SkillsSettingsSection
          config={settings.config}
          onConfigChange={settings.updateConfig}
        />
      );
    case "git":
      return (
        <GitSettingsPanel
          config={settings.config!}
          onConfigChange={settings.updateConfig}
        />
      );
    case "appearance":
      return (
        <AppearanceSettingsSection
          theme={theme}
          onThemeChange={onThemeChange}
        />
      );
    case "memory":
      return (
        <MemorySettingsSection
          config={settings.config}
          onConfigChange={settings.updateConfig}
        />
      );
    case "hooks":
      return (
        <HooksSettingsSection
          config={settings.config!}
          onConfigChange={settings.updateConfig}
        />
      );
    case "mcp":
      return <McpSettingsSection />;
    case "usage":
      return <UsageStatsSection />;
    case "gateways":
      return (
        <GatewaySettingsSection
          config={settings.config!}
          dirty={settings.dirty}
          onGatewayChange={settings.updateGateway}
          onSave={settings.saveConfig}
        />
      );
    case "advanced":
      return (
        <AdvancedSettingsSection
          value={settings.raw}
          onChange={settings.updateRaw}
        />
      );
    default:
      return null;
  }
}

/**
 * 判断 section 是否必须等待全局 AppConfig。
 *
 * @param section section 标识
 * @returns 需要 config 时 true
 */
function sectionNeedsAppConfig(section: SettingsSectionId): boolean {
  return (
    section === "providers"
    || section === "agents"
    || section === "plugins"
    || section === "runtime"
    || section === "git"
    || section === "hooks"
    || section === "gateways"
    || section === "advanced"
  );
}
