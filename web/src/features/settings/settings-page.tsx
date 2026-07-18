import { ArrowLeft, Save } from "lucide-react";
import { useState } from "react";
import { Link } from "react-router-dom";
import { AdvancedSettingsSection } from "./advanced-settings-section";
import { AgentSettingsSection } from "./agents/agent-settings-section";
import { AppearanceSettingsSection } from "./appearance-settings-section";
import { GatewaySettingsSection } from "./gateway-settings-section";
import { ProviderSettingsSection } from "./provider-settings-section";
import { PluginSettingsSection } from "./plugin-settings-section";
import { RuntimeSettingsSection } from "./runtime-settings-section";
import { MemorySettingsSection } from "./memory-settings-section";
import { HooksMcpSettingsSection } from "./hooks-mcp-settings-section";
import { SaveStatusBadge } from "./save-status-badge";
import { SETTINGS_SECTIONS } from "./settings-sections";
import type { SettingsSectionId } from "./settings-types";
import { useSettingsConfig } from "./use-settings-config";
import { useTheme } from "../theme/theme";
import { useI18n } from "../i18n/use-i18n";
import "./settings-layout.css";
import "./settings-forms.css";
import "./settings-catalog.css";
import "./settings-sections.css";

/**
 * 渲染应用配置页面，含紧凑吸顶标题条、分类导航和编辑区。
 *
 * @returns 设置页面
 */
export function SettingsPage() {
  const settings = useSettingsConfig();
  const theme = useTheme();
  const { t } = useI18n();
  const [section, setSection] = useState<SettingsSectionId>("providers");

  return (
    <div className="settings-page">
      <header className="settings-topbar">
        <div className="settings-topbar-inner">
          <Link to="/" className="settings-back" aria-label={t("Back to workspace", "返回主界面")}><ArrowLeft size={15} /><span>{t("Back to workspace", "返回主界面")}</span></Link>
          <h1>{t("Settings", "设置")}</h1>
          <p>{t("Manage models, plugins, prompts, tools, gateways, and interface preferences.", "管理模型、插件、提示词、工具、网关和界面偏好。")}</p>
          <div className="settings-topbar-actions">
            <SaveStatusBadge dirty={settings.dirty} saving={settings.saving} saveError={Boolean(settings.error)} loaded={Boolean(settings.config)} />
            <button
              type="button"
              className="settings-save"
              onClick={() => void settings.saveConfig()}
              disabled={!settings.config || !settings.dirty || settings.saving}
            >
              <Save size={14} />{settings.saving ? t("Saving", "正在保存") : t("Save changes", "保存修改")}
            </button>
          </div>
        </div>
      </header>
      <div className="settings-workspace">
        <nav className="settings-navigation" aria-label={t("Settings categories", "设置分类")}>
          <div className="settings-navigation-label">{t("Settings categories", "设置分类")}</div>
          {SETTINGS_SECTIONS.map(({ id, labelEn, labelZh, descriptionEn, descriptionZh, icon: Icon }) => (
            <button type="button" key={id} className={id === section ? "active" : ""} onClick={() => setSection(id)}>
              <Icon size={15} />
              <span><strong>{t(labelEn, labelZh)}</strong><small>{t(descriptionEn, descriptionZh)}</small></span>
            </button>
          ))}
        </nav>
        <main className="settings-main">
          {settings.loading && <div className="settings-state">{t("Loading configuration", "正在读取配置")}</div>}
          {settings.config && section === "providers" && <ProviderSettingsSection config={settings.config} onConfigChange={settings.updateConfig} onProviderChange={settings.updateProvider} />}
          {settings.config && section === "agents" && <AgentSettingsSection config={settings.config} onConfigChange={settings.updateConfig} />}
          {settings.config && section === "plugins" && <PluginSettingsSection config={settings.config} onConfigChange={settings.updateConfig} />}
          {settings.config && section === "runtime" && <RuntimeSettingsSection config={settings.config} onConfigChange={settings.updateConfig} />}
          {section === "appearance" && <AppearanceSettingsSection theme={theme.theme} onThemeChange={theme.setTheme} />}
          {section === "memory" && <MemorySettingsSection />}
          {settings.config && section === "hooks" && <HooksMcpSettingsSection config={settings.config} onConfigChange={settings.updateConfig} />}
          {settings.config && section === "gateways" && <GatewaySettingsSection config={settings.config} dirty={settings.dirty} onGatewayChange={settings.updateGateway} onSave={settings.saveConfig} />}
          {settings.config && section === "advanced" && <AdvancedSettingsSection value={settings.raw} onChange={settings.updateRaw} />}
          {settings.error && <div className="settings-error">{settings.error.message}</div>}
        </main>
      </div>
    </div>
  );
}
