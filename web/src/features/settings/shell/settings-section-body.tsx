import type { ReactNode } from "react";
import type { AppConfig } from "../../../api/contracts";
import { AdvancedSettingsSection } from "../advanced-settings-section";
import { AgentSettingsSection } from "../agents/agent-settings-section";
import { AppearanceSettingsSection } from "../appearance-settings-section";
import { GatewaySettingsSection } from "../gateway-settings-section";
import { GitSettingsPanel } from "../git/git-settings-panel";
import { SshSettingsSection } from "../ssh/ssh-settings-section";
import { ProviderSettingsSection } from "../provider-settings-section";
import { CliToolsSettingsSection } from "../cli-tools/cli-tools-settings-section";
import { WebSearchSettingsSection } from "../web-search/web-search-settings-section";
import { RuntimeSettingsSection } from "../runtime-settings-section";
import { MemorySettingsSection } from "../memory-settings-section";
import { HooksSettingsSection } from "../hooks-settings-section";
import { McpSettingsSection } from "../mcp/mcp-settings-section";
import { SkillsSettingsSection } from "../skills/skills-settings-section";
import { UsageStatsSection } from "../usage/usage-stats-section";
import { SessionDataSettings } from "../session-data/session-data-settings";
import { SettingsSkeleton } from "./settings-skeleton";
import { SettingsErrorRecovery } from "./settings-error-recovery";
import { getSettingsSection } from "../settings-registry";
import type { SettingsConfigController, SettingsSectionId } from "../settings-types";
import type { ThemeId } from "../../theme/theme";
import { useI18n } from "../../i18n/use-i18n";
import { PromptTemplateSettings } from "../prompts/prompt-template-settings";
import { resolvePromptTemplates } from "../prompts/prompt-template-catalog";

type SettingsSectionBodyProps = {
  section: SettingsSectionId;
  /** 当前二级子页；无子页分区为 undefined */
  subview?: string;
  settings: SettingsConfigController;
  theme: ThemeId;
  onThemeChange: (theme: ThemeId) => void;
};

/**
 * 按 section id 挂载对应设置面板。
 *
 * appConfig 为 required 的分区必须等全局配置就绪：加载中出骨架、
 * 失败出恢复面板，随后以非空 config 渲染；其余分区直接渲染。
 *
 * @param props 当前 section、全局配置控制器与外观偏好
 * @returns section 内容
 */
export function SettingsSectionBody({
  section,
  subview,
  settings,
  theme,
  onThemeChange
}: SettingsSectionBodyProps) {
  const { t } = useI18n();
  const requiresConfig = getSettingsSection(section)?.appConfig === "required";

  if (requiresConfig) {
    // 1. 必需 AppConfig 的分区：加载中展示骨架屏
    if (settings.loading) {
      return <SettingsSkeleton rows={5} />;
    }
    // 2. 加载失败且无缓存配置时展示错误恢复面板
    if (!settings.config) {
      const message = settings.error?.message
        ?? t("Configuration unavailable", "配置不可用");
      return <SettingsErrorRecovery message={message} onRetry={settings.retry} />;
    }
    return renderAppConfigSection(section, subview, settings.config, settings);
  }
  return renderStandaloneSection(section, subview, settings, theme, onThemeChange);
}

/**
 * 渲染必需全局 AppConfig 的分区。
 *
 * config 在入口处已收窄为非空，各分支不再需要断言。
 *
 * @param section 当前 section
 * @param config 已加载的应用配置
 * @param settings 全局配置控制器
 * @returns section 内容
 */
function renderAppConfigSection(
  section: SettingsSectionId,
  subview: string | undefined,
  config: AppConfig,
  settings: SettingsConfigController
): ReactNode {
  switch (section) {
    case "providers":
      return (
        <ProviderSettingsSection
          config={config}
          subview={subview}
          secretSentinel={settings.secretSentinel}
          onConfigChange={settings.updateConfig}
          onProviderChange={settings.updateProvider}
        />
      );
    case "agents":
      return (
        <AgentSettingsSection
          config={config}
          onConfigChange={settings.updateConfig}
        />
      );
    case "cli-tools":
      return (
        <CliToolsSettingsSection
          config={config}
          secretSentinel={settings.secretSentinel}
          onConfigChange={settings.updateConfig}
        />
      );
    case "web-search":
      return (
        <WebSearchSettingsSection
          config={config}
          secretSentinel={settings.secretSentinel}
          onConfigChange={settings.updateConfig}
        />
      );
    case "runtime":
      return (
        <RuntimeSettingsSection
          config={config}
          subview={subview}
          onConfigChange={settings.updateConfig}
        />
      );
    case "prompts":
      return (
        <PromptTemplateSettings
          templates={resolvePromptTemplates(config.prompt?.templates)}
          onChange={(templates) => settings.updateConfig({
            ...config,
            prompt: { ...config.prompt, templates }
          })}
        />
      );
    case "git":
      return (
        <GitSettingsPanel
          config={config}
          onConfigChange={settings.updateConfig}
        />
      );
    case "ssh":
      return <SshSettingsSection />;
    case "hooks":
      return (
        <HooksSettingsSection
          config={config}
          onConfigChange={settings.updateConfig}
        />
      );
    case "gateways":
      return (
        <GatewaySettingsSection
          config={config}
          dirty={settings.dirty}
          onGatewayChange={settings.updateGateway}
          onSave={settings.saveConfig}
        />
      );
    case "advanced":
      return (
        <AdvancedSettingsSection
          config={config}
          onConfigChange={settings.updateConfig}
        />
      );
    default:
      return null;
  }
}

/**
 * 渲染不阻塞于全局 AppConfig 的分区。
 *
 * optional 面（skills / memory）接受可空 config 内部降级，
 * none 面完全不读写全局配置。
 *
 * @param section 当前 section
 * @param settings 全局配置控制器
 * @param theme 当前主题
 * @param onThemeChange 主题切换回调
 * @returns section 内容
 */
function renderStandaloneSection(
  section: SettingsSectionId,
  subview: string | undefined,
  settings: SettingsConfigController,
  theme: ThemeId,
  onThemeChange: (theme: ThemeId) => void
): ReactNode {
  switch (section) {
    case "appearance":
      return (
        <AppearanceSettingsSection
          theme={theme}
          onThemeChange={onThemeChange}
        />
      );
    case "skills":
      return (
        <SkillsSettingsSection
          config={settings.config}
          onConfigChange={settings.updateConfig}
        />
      );
    case "memory":
      return (
        <MemorySettingsSection
          config={settings.config}
          onConfigChange={settings.updateConfig}
        />
      );
    case "mcp":
      return <McpSettingsSection />;
    case "usage":
      return <UsageStatsSection subview={subview} />;
    case "session-data":
      return <SessionDataSettings />;
    default:
      return null;
  }
}
