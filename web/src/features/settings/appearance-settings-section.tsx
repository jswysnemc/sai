import type { ThemeId } from "../theme/theme";
import { useI18n } from "../i18n/use-i18n";
import { useMarkdownStylePreferences } from "../markdown/markdown-style-store";
import { ChoicePills, SettingsField, SettingsPanel } from "./kit";
import { DisplayScaleSettings } from "./appearance/display-scale-settings";
import { ThemeSettings } from "./appearance/theme-settings";
import { MarkdownStyleSettings } from "./markdown-style-settings";

/**
 * 【Web 设置】【界面外观】组合语言、主题和 Markdown 偏好，所有修改即时生效。
 * @param props 当前主题及切换回调
 * @returns 当前浏览器的外观设置
 */
export function AppearanceSettingsSection({ theme, onThemeChange }: { theme: ThemeId; onThemeChange: (theme: ThemeId) => void }) {
  const { locale, setLocale, t } = useI18n();
  const markdownStyle = useMarkdownStylePreferences();
  return (
    <>
      <SettingsPanel title={t("Language", "语言")} description={t("Preferences apply immediately and stay in this browser.", "界面偏好即时应用，仅保存在当前浏览器。")}>
        <SettingsField label={t("Interface language", "界面语言")} anchor="appearance.locale">
          <ChoicePills value={locale} options={[{ value: "zh-CN", label: "简体中文" }, { value: "en-US", label: "English" }]} onChange={setLocale} />
        </SettingsField>
      </SettingsPanel>
      <ThemeSettings theme={theme} onThemeChange={onThemeChange} />
      <DisplayScaleSettings />
      <MarkdownStyleSettings preferences={markdownStyle.preferences} onPresetChange={markdownStyle.updatePreset} onTableChange={markdownStyle.updateTable} onCodeBlockChange={markdownStyle.updateCodeBlock} onReset={markdownStyle.reset} />
    </>
  );
}
