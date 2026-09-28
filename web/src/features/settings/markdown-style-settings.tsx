import { Check, RotateCcw } from "../../shared/ui/icons";
import { Button } from "../../shared/ui/button/button";
import { useI18n } from "../i18n/use-i18n";
import { SettingsField, SettingsPanel } from "./kit";
import { AppearancePreview } from "./appearance-preview/appearance-preview";
import { MarkdownTableSettings } from "./appearance/markdown-table-settings";
import { MarkdownCodeSettings } from "./appearance/markdown-code-settings";
import { PRESET_OPTIONS } from "./appearance/markdown-preset-options";
import type { MarkdownStyleSettingsProps } from "./appearance/markdown-settings-types";
import "./markdown-style-settings.css";

/**
 * 【Web 设置】【Markdown 外观】组合整体风格、表格、代码块和实时预览。
 * @param props 当前偏好、各领域更新回调与恢复回调
 * @returns Markdown 外观设置
 */
export function MarkdownStyleSettings({ preferences, onPresetChange, onTableChange, onCodeBlockChange, onReset }: MarkdownStyleSettingsProps) {
  const { t } = useI18n();
  return (
    <>
      <SettingsPanel title={t("Markdown rendering", "Markdown 渲染")} description={t("Changes apply immediately to rendered Markdown in this browser.", "修改后立即应用到当前浏览器中的 Markdown 内容。")} actions={<Button size="small" onClick={onReset}><RotateCcw size={14} />{t("Reset", "恢复默认")}</Button>}>
        <SettingsField label={t("Overall style", "整体风格")} anchor="appearance.markdown.preset">
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-4" role="group" aria-label={t("Markdown style preset", "Markdown 风格预设")}>
            {PRESET_OPTIONS.map((option) => (
              <Button
                className={option.value === preferences.preset ? "markdown-preset active" : "markdown-preset"}
                onClick={() => onPresetChange(option.value)}
                aria-pressed={option.value === preferences.preset}
                key={option.value}
              >
                <Check size={12} className="markdown-preset-check" />
                <strong>{t(option.nameEn, option.nameZh)}</strong>
                <small>{t(option.descriptionEn, option.descriptionZh)}</small>
              </Button>
            ))}
          </div>
        </SettingsField>
      </SettingsPanel>
      <MarkdownTableSettings preferences={preferences} onTableChange={onTableChange} />
      <MarkdownCodeSettings preferences={preferences} onCodeBlockChange={onCodeBlockChange} />
      <AppearancePreview preferences={preferences} />
    </>
  );
}
