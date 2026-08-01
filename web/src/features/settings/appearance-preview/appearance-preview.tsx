import { useMemo } from "react";
import type { MarkdownStylePreferences } from "../../markdown/markdown-style-preferences";
import { MarkdownRenderer } from "../../chat/markdown-renderer";
import { useI18n } from "../../i18n/use-i18n";
import { buildAppearancePreviewSample } from "./appearance-preview-sample";
import "./appearance-preview.css";

type AppearancePreviewProps = {
  /** 当前编辑中的外观配置 */
  preferences: MarkdownStylePreferences;
};

/**
 * 用当前外观配置实时渲染一段示例内容。
 *
 * 表格边框、代码行号这类选项只看文字描述难以判断效果，这里直接把待生效的
 * 配置交给正式渲染器，所见即为聊天中的实际呈现。
 *
 * @param props 当前编辑中的外观配置
 * @returns 示例渲染面板
 */
export function AppearancePreview({ preferences }: AppearancePreviewProps) {
  const { locale, t } = useI18n();
  const sample = useMemo(() => buildAppearancePreviewSample(locale), [locale]);

  return (
    <section className="appearance-preview" aria-label={t("Preview", "效果预览")}>
      <header className="appearance-preview-head">
        <strong>{t("Preview", "效果预览")}</strong>
        <small>{t("Reflects the settings above.", "实时反映上方设置。")}</small>
      </header>
      <div className="appearance-preview-body">
        <MarkdownRenderer source={sample} stylePreferences={preferences} />
      </div>
    </section>
  );
}
