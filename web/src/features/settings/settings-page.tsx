import { ArrowLeft } from "lucide-react";
import { Link, Navigate, useParams } from "react-router-dom";
import { SettingsNav } from "./shell/settings-nav";
import { SettingsSaveBar } from "./shell/settings-save-bar";
import { SettingsSectionBody } from "./shell/settings-section-body";
import { SettingsSubnav } from "./shell/settings-subnav";
import {
  getSettingsSection,
  resolveSettingsSectionId,
  resolveSettingsSubview
} from "./settings-registry";
import { useSettingsConfig } from "./use-settings-config";
import { useTheme } from "../theme/theme";
import { useI18n } from "../i18n/use-i18n";
import "./settings-layout.css";
import "./settings-forms.css";
import "./settings-catalog.css";

/**
 * 设置页壳层：顶栏、分组导航、按路由挂载 section 与子页。
 *
 * @returns 设置页面
 */
export function SettingsPage() {
  const params = useParams<{ sectionId?: string; subview?: string }>();
  const requested = params.sectionId;
  const section = resolveSettingsSectionId(requested);
  const meta = getSettingsSection(section);
  const subview = resolveSettingsSubview(meta, params.subview);
  const settings = useSettingsConfig();
  const theme = useTheme();
  const { t } = useI18n();

  // 1. 归一非法 section 与子页段：有子页的分区始终落在显式子页 URL 上
  if (!requested || requested !== section || (params.subview ?? undefined) !== subview) {
    return <Navigate to={subview ? `/settings/${section}/${subview}` : `/settings/${section}`} replace />;
  }

  return (
    <div className="settings-page">
      <header className="settings-topbar">
        <div className="settings-topbar-inner">
          <Link to="/" className="settings-back" aria-label={t("Back to workspace", "返回主界面")}>
            <ArrowLeft size={15} />
            <span>{t("Back to workspace", "返回主界面")}</span>
          </Link>
          <h1>{t("Settings", "设置")}</h1>
          <p>
            {meta
              ? t(meta.descriptionEn, meta.descriptionZh)
              : t(
                  "Manage models, CLI assistant tools, agents, gateways, and interface preferences.",
                  "管理模型、CLI 助手工具、Agent、网关和界面偏好。"
                )}
          </p>
          <div className="settings-topbar-actions">
            <SettingsSaveBar
              meta={meta}
              dirty={settings.dirty}
              saving={settings.saving}
              saveError={Boolean(settings.error)}
              loaded={Boolean(settings.config)}
              onSave={() => void settings.saveConfig()}
            />
          </div>
        </div>
      </header>
      <div className="settings-workspace">
        <SettingsNav activeSection={section} />
        <main className="settings-main">
          {meta?.subviews && (
            <SettingsSubnav sectionId={section} subviews={meta.subviews} />
          )}
          <SettingsSectionBody
            section={section}
            subview={subview}
            settings={settings}
            theme={theme.theme}
            onThemeChange={theme.setTheme}
          />
          {settings.error && meta && meta.appConfig !== "none" && (
            <div className="settings-error">{settings.error.message}</div>
          )}
        </main>
      </div>
    </div>
  );
}
