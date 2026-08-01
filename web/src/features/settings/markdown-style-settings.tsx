import { Code2, RotateCcw, Table2 } from "lucide-react";
import type {
  MarkdownCodeBlockStylePreferences,
  MarkdownStylePreferences,
  MarkdownTableStylePreferences
} from "../markdown/markdown-style-preferences";
import { useI18n } from "../i18n/use-i18n";
import { Button } from "../../shared/ui/button/button";
import { Select } from "../../shared/ui/select/select";
import { AppearancePreview } from "./appearance-preview/appearance-preview";
import { SettingsGroup } from "./editor-layout";
import "./markdown-style-settings.css";

type MarkdownStyleSettingsProps = {
  preferences: MarkdownStylePreferences;
  onTableChange: (patch: Partial<MarkdownTableStylePreferences>) => void;
  onCodeBlockChange: (patch: Partial<MarkdownCodeBlockStylePreferences>) => void;
  onReset: () => void;
};

/**
 * 渲染 Markdown 表格与代码块的细粒度外观设置。
 *
 * @param props 当前偏好、局部更新回调和重置回调
 * @returns Markdown 外观设置分组
 */
export function MarkdownStyleSettings({
  preferences,
  onTableChange,
  onCodeBlockChange,
  onReset
}: MarkdownStyleSettingsProps) {
  const { t } = useI18n();

  return (
    <SettingsGroup
      title={t("Markdown rendering", "Markdown 渲染")}
      description={t(
        "Tune table structure and code block readability. Changes apply to all rendered Markdown immediately.",
        "细化表格结构与代码块可读性，修改后立即应用到全部 Markdown 内容。"
      )}
      actions={(
        <Button variant="secondary" onClick={onReset}>
          <RotateCcw size={14} />
          {t("Reset", "恢复默认")}
        </Button>
      )}
    >
      <div className="markdown-style-grid">
        <fieldset className="markdown-style-panel">
          <legend><span><Table2 size={15} />{t("Tables", "表格")}</span></legend>
          <div className="markdown-style-fields">
            <label className="settings-field">
              <span>{t("Table borders", "表格边框")}</span>
              <Select
                value={preferences.table.borderStyle}
                options={[
                  { value: "horizontal", label: t("Horizontal lines", "仅横向分隔线") },
                  { value: "grid", label: t("Full grid", "完整网格") },
                  { value: "none", label: t("No borders", "无边框") }
                ]}
                ariaLabel={t("Table borders", "表格边框")}
                onChange={(borderStyle) => onTableChange({ borderStyle })}
              />
            </label>
            <label className="settings-field">
              <span>{t("Cell density", "单元格密度")}</span>
              <Select
                value={preferences.table.density}
                options={[
                  { value: "compact", label: t("Compact", "紧凑") },
                  { value: "comfortable", label: t("Comfortable", "标准") },
                  { value: "spacious", label: t("Spacious", "宽松") }
                ]}
                ariaLabel={t("Cell density", "单元格密度")}
                onChange={(density) => onTableChange({ density })}
              />
            </label>
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Full width", "占满内容宽度")}</strong>
                <small>{t("Stretch short tables to the message width.", "短表格也扩展到消息内容宽度。")}</small>
              </span>
              <input
                type="checkbox"
                checked={preferences.table.fullWidth}
                onChange={(event) => onTableChange({ fullWidth: event.target.checked })}
              />
            </label>
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Striped rows", "斑马纹")}</strong>
                <small>{t("Add a subtle surface to alternating rows.", "为交替数据行增加轻微底色。")}</small>
              </span>
              <input
                type="checkbox"
                checked={preferences.table.stripedRows}
                onChange={(event) => onTableChange({ stripedRows: event.target.checked })}
              />
            </label>
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Header background", "表头底色")}</strong>
                <small>{t("Separate the header with a muted surface.", "使用克制底色区分表头。")}</small>
              </span>
              <input
                type="checkbox"
                checked={preferences.table.headerBackground}
                onChange={(event) => onTableChange({ headerBackground: event.target.checked })}
              />
            </label>
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Wrap cell content", "单元格内容换行")}</strong>
                <small>{t("Wrap long text instead of keeping every cell on one line.", "长文本可以换行，不强制每个单元格保持单行。")}</small>
              </span>
              <input
                type="checkbox"
                checked={preferences.table.wrapCells}
                onChange={(event) => onTableChange({ wrapCells: event.target.checked })}
              />
            </label>
          </div>
        </fieldset>

        <fieldset className="markdown-style-panel">
          <legend><span><Code2 size={15} />{t("Code blocks", "代码块")}</span></legend>
          <div className="markdown-style-fields">
            <label className="settings-field">
              <span>{t("Font size", "代码字体大小")}</span>
              <Select
                value={preferences.codeBlock.fontSize}
                options={[
                  { value: "small", label: t("Small", "较小") },
                  { value: "medium", label: t("Medium", "标准") },
                  { value: "large", label: t("Large", "较大") }
                ]}
                ariaLabel={t("Code font size", "代码字体大小")}
                onChange={(fontSize) => onCodeBlockChange({ fontSize })}
              />
            </label>
            <label className="settings-field">
              <span>{t("Tab width", "制表符宽度")}</span>
              <Select
                value={preferences.codeBlock.tabSize}
                options={[
                  { value: "2", label: t("2 spaces", "2 个空格") },
                  { value: "4", label: t("4 spaces", "4 个空格") },
                  { value: "8", label: t("8 spaces", "8 个空格") }
                ]}
                ariaLabel={t("Tab width", "制表符宽度")}
                onChange={(tabSize) => onCodeBlockChange({ tabSize })}
              />
            </label>
            <label className="settings-field">
              <span>{t("Maximum height", "最大高度")}</span>
              <Select
                value={preferences.codeBlock.maxHeight}
                options={[
                  { value: "none", label: t("No limit", "不限制") },
                  { value: "medium", label: t("Medium · 24 rem", "中等 · 24rem") },
                  { value: "tall", label: t("Tall · 36 rem", "较高 · 36rem") }
                ]}
                ariaLabel={t("Code block maximum height", "代码块最大高度")}
                onChange={(maxHeight) => onCodeBlockChange({ maxHeight })}
              />
            </label>
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Line numbers", "显示行号")}</strong>
                <small>{t("Show a fixed number column for every source line.", "为每一行源码显示连续编号。")}</small>
              </span>
              <input
                type="checkbox"
                checked={preferences.codeBlock.lineNumbers}
                onChange={(event) => onCodeBlockChange({ lineNumbers: event.target.checked })}
              />
            </label>
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Wrap long lines", "长行换行")}</strong>
                <small>{t("Wrap long source lines instead of scrolling horizontally.", "长源码行自动折行，不使用横向滚动。")}</small>
              </span>
              <input
                type="checkbox"
                checked={preferences.codeBlock.wrapLongLines}
                onChange={(event) => onCodeBlockChange({ wrapLongLines: event.target.checked })}
              />
            </label>
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Language label", "语言标签")}</strong>
                <small>{t("Show the detected language in the code block header.", "在代码块头部显示识别到的语言。")}</small>
              </span>
              <input
                type="checkbox"
                checked={preferences.codeBlock.showLanguageLabel}
                onChange={(event) => onCodeBlockChange({ showLanguageLabel: event.target.checked })}
              />
            </label>
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Copy button", "复制按钮")}</strong>
                <small>{t("Show the copy action in the code block header.", "在代码块头部显示复制操作。")}</small>
              </span>
              <input
                type="checkbox"
                checked={preferences.codeBlock.showCopyButton}
                onChange={(event) => onCodeBlockChange({ showCopyButton: event.target.checked })}
              />
            </label>
            <label className="settings-toggle-field">
              <span>
                <strong>{t("Block border", "代码块外框")}</strong>
                <small>{t("Add a thin neutral border around code blocks.", "为代码块增加同色系细边框。")}</small>
              </span>
              <input
                type="checkbox"
                checked={preferences.codeBlock.showBorder}
                onChange={(event) => onCodeBlockChange({ showBorder: event.target.checked })}
              />
            </label>
          </div>
        </fieldset>
      </div>
      <AppearancePreview preferences={preferences} />
    </SettingsGroup>
  );
}
