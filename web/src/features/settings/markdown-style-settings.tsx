import { Check, Code2, LayoutTemplate, RotateCcw, Table2 } from "../../shared/ui/icons";
import type {
  MarkdownCodeBlockStylePreferences,
  MarkdownStylePreferences,
  MarkdownStylePreset,
  MarkdownTableStylePreferences
} from "../markdown/markdown-style-preferences";
import { useI18n } from "../i18n/use-i18n";
import { Button } from "../../shared/ui/button/button";
import { Select } from "../../shared/ui/select/select";
import { AppearancePreview } from "./appearance-preview/appearance-preview";
import { ToggleRow } from "./controls/toggle-row";
import "./markdown-style-settings.css";

type MarkdownStyleSettingsProps = {
  preferences: MarkdownStylePreferences;
  onPresetChange: (preset: MarkdownStylePreset) => void;
  onTableChange: (patch: Partial<MarkdownTableStylePreferences>) => void;
  onCodeBlockChange: (patch: Partial<MarkdownCodeBlockStylePreferences>) => void;
  onReset: () => void;
};

type PresetOption = {
  value: MarkdownStylePreset;
  nameEn: string;
  nameZh: string;
  descriptionEn: string;
  descriptionZh: string;
};

const PRESET_OPTIONS: readonly PresetOption[] = [
  {
    value: "default",
    nameEn: "Default",
    nameZh: "默认",
    descriptionEn: "Balanced spacing and neutral tones.",
    descriptionZh: "均衡间距与中性配色。"
  },
  {
    value: "compact",
    nameEn: "Compact",
    nameZh: "紧凑",
    descriptionEn: "Denser headings and tighter line height.",
    descriptionZh: "更小标题与更紧的行距，信息密度优先。"
  },
  {
    value: "document",
    nameEn: "Document",
    nameZh: "文档",
    descriptionEn: "Generous whitespace for long-form reading.",
    descriptionZh: "更大留白与行高，适合长文阅读。"
  },
  {
    value: "vivid",
    nameEn: "Vivid",
    nameZh: "彩色",
    descriptionEn: "Accent-colored headings, markers, and quotes.",
    descriptionZh: "标题、列表与引用带主题色视觉锚点。"
  }
];

/**
 * 渲染 Markdown 表格与代码块的细粒度外观设置。
 *
 * @param props 当前偏好、局部更新回调和重置回调
 * @returns Markdown 外观设置分组
 */
export function MarkdownStyleSettings({
  preferences,
  onPresetChange,
  onTableChange,
  onCodeBlockChange,
  onReset
}: MarkdownStyleSettingsProps) {
  const { t } = useI18n();

  return (
    <section className="markdown-config-root">
      <header className="markdown-config-head">
        <div>
          <h3>{t("Markdown rendering", "Markdown 渲染")}</h3>
          <p>{t(
            "Pick an overall style, then tune table structure and code block readability. Changes apply to all rendered Markdown immediately.",
            "先选择整体风格，再细化表格结构与代码块可读性，修改后立即应用到全部 Markdown 内容。"
          )}</p>
        </div>
        <Button variant="secondary" onClick={onReset}>
          <RotateCcw size={14} />
          {t("Reset", "恢复默认")}
        </Button>
      </header>
      <fieldset className="markdown-style-panel markdown-preset-panel">
        <legend><span><LayoutTemplate size={16} />{t("Overall style", "整体风格")}</span></legend>
        <div className="markdown-preset-grid" role="group" aria-label={t("Markdown style preset", "Markdown 风格预设")}>
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
      </fieldset>
      <div className="markdown-config-columns">
        <fieldset className="markdown-style-panel">
          <legend><span><Table2 size={16} />{t("Tables", "表格")}</span></legend>
          <div className="markdown-config-stack">
            <label className="markdown-config-row">
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
            <label className="markdown-config-row">
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
            <ToggleRow
              className="markdown-config-toggle"
              label={t("Full width", "占满内容宽度")}
              hint={t("Stretch short tables to the message width.", "短表格也扩展到消息内容宽度。")}
              checked={preferences.table.fullWidth}
              onChange={(fullWidth) => onTableChange({ fullWidth })}
            />
            <ToggleRow
              className="markdown-config-toggle"
              label={t("Striped rows", "斑马纹")}
              hint={t("Add a subtle surface to alternating rows.", "为交替数据行增加轻微底色。")}
              checked={preferences.table.stripedRows}
              onChange={(stripedRows) => onTableChange({ stripedRows })}
            />
            <ToggleRow
              className="markdown-config-toggle"
              label={t("Header background", "表头底色")}
              hint={t("Separate the header with a muted surface.", "使用克制底色区分表头。")}
              checked={preferences.table.headerBackground}
              onChange={(headerBackground) => onTableChange({ headerBackground })}
            />
            <ToggleRow
              className="markdown-config-toggle"
              label={t("Wrap cell content", "单元格内容换行")}
              hint={t("Wrap long text instead of keeping every cell on one line.", "长文本可以换行，不强制每个单元格保持单行。")}
              checked={preferences.table.wrapCells}
              onChange={(wrapCells) => onTableChange({ wrapCells })}
            />
          </div>
        </fieldset>

        <fieldset className="markdown-style-panel">
          <legend><span><Code2 size={16} />{t("Code blocks", "代码块")}</span></legend>
          <div className="markdown-config-stack">
            <label className="markdown-config-row">
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
            <label className="markdown-config-row">
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
            <label className="markdown-config-row">
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
            <ToggleRow
              className="markdown-config-toggle"
              label={t("Line numbers", "显示行号")}
              hint={t("Show a fixed number column for every source line.", "为每一行源码显示连续编号。")}
              checked={preferences.codeBlock.lineNumbers}
              onChange={(lineNumbers) => onCodeBlockChange({ lineNumbers })}
            />
            <ToggleRow
              className="markdown-config-toggle"
              label={t("Wrap long lines", "长行换行")}
              hint={t("Wrap long source lines instead of scrolling horizontally.", "长源码行自动折行，不使用横向滚动。")}
              checked={preferences.codeBlock.wrapLongLines}
              onChange={(wrapLongLines) => onCodeBlockChange({ wrapLongLines })}
            />
            <ToggleRow
              className="markdown-config-toggle"
              label={t("Language label", "语言标签")}
              hint={t("Show the detected language in the code block header.", "在代码块头部显示识别到的语言。")}
              checked={preferences.codeBlock.showLanguageLabel}
              onChange={(showLanguageLabel) => onCodeBlockChange({ showLanguageLabel })}
            />
            <ToggleRow
              className="markdown-config-toggle"
              label={t("Copy button", "复制按钮")}
              hint={t("Show the copy action in the code block header.", "在代码块头部显示复制操作。")}
              checked={preferences.codeBlock.showCopyButton}
              onChange={(showCopyButton) => onCodeBlockChange({ showCopyButton })}
            />
            <ToggleRow
              className="markdown-config-toggle"
              label={t("Block border", "代码块外框")}
              hint={t("Add a thin neutral border around code blocks.", "为代码块增加同色系细边框。")}
              checked={preferences.codeBlock.showBorder}
              onChange={(showBorder) => onCodeBlockChange({ showBorder })}
            />
          </div>
        </fieldset>
      </div>
      <AppearancePreview preferences={preferences} />
    </section>
  );
}
