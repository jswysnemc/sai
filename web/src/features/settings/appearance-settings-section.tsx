import { Check } from "lucide-react";
import { EditorHeader } from "./editor-layout";
import { SettingsGroup } from "./editor-layout";
import type { ThemeId } from "../theme/theme";
import { THEME_PRESETS } from "../theme/theme";
import { useI18n } from "../i18n/use-i18n";
import { Select } from "../../shared/ui/select/select";

type AppearanceSettingsSectionProps = {
  theme: ThemeId;
  onThemeChange: (theme: ThemeId) => void;
};

/**
 * 渲染主题配色选择区域。
 *
 * @param props 当前主题和更新回调
 * @returns 外观设置区域
 */
export function AppearanceSettingsSection({ theme, onThemeChange }: AppearanceSettingsSectionProps) {
  const { locale, setLocale, t } = useI18n();

  return (
    <section className="settings-editor">
      <EditorHeader
        kicker={t("Interface", "界面外观")}
        title={t("Language and appearance", "语言与主题")}
        description={t(
          "Preferences apply immediately and are stored in this browser without changing server configuration.",
          "界面偏好即时应用并保存在当前浏览器，不修改服务端配置。"
        )}
      />
      <SettingsGroup
        title={t("Interface language", "界面语言")}
        description={t("Choose the language used by the Web interface.", "选择 Web 界面使用的语言。")}
      >
        <label className="settings-field">
          <span>{t("Language", "语言")}</span>
          <Select
            value={locale}
            options={[
              { value: "zh-CN", label: "简体中文", description: t("Chinese (Simplified)", "简体中文") },
              { value: "en-US", label: "English", description: t("English", "英语") }
            ]}
            ariaLabel={t("Interface language", "界面语言")}
            onChange={setLocale}
          />
        </label>
      </SettingsGroup>
      <SettingsGroup
        title={t("Theme and colors", "主题与配色")}
        description={t("Choose a compact color scheme for the workspace.", "选择适合工作区的紧凑配色方案。")}
      >
        <div className="theme-preset-grid">
          {THEME_PRESETS.map((preset) => (
            <button
              type="button"
              className={preset.id === theme ? "theme-preset active" : "theme-preset"}
              onClick={() => onThemeChange(preset.id)}
              aria-pressed={preset.id === theme}
              key={preset.id}
            >
              <span className="theme-swatches">
                {preset.colors.map((color) => <i style={{ background: color }} key={color} />)}
              </span>
              <span className="theme-preset-copy">
                <strong>{t(preset.nameEn, preset.nameZh)}</strong>
                <small>{t(preset.descriptionEn, preset.descriptionZh)}</small>
              </span>
              <Check size={15} className="theme-preset-check" />
            </button>
          ))}
        </div>
      </SettingsGroup>
    </section>
  );
}
