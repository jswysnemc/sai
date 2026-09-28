import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, FieldGrid, SettingsField, SettingsPanel, SwitchField } from "../kit";
import type { MarkdownStyleSettingsProps } from "./markdown-settings-types";

/**
 * 【Web 设置】【Markdown 外观】渲染领域独立的外观字段。
 * @param props 当前偏好与对应领域更新回调
 * @returns 外观设置面板
 */
export function MarkdownCodeSettings({ preferences, onCodeBlockChange }: Pick<MarkdownStyleSettingsProps, "preferences" | "onCodeBlockChange">) {
  const { t } = useI18n();
  return (
    <SettingsPanel title={t("Code blocks", "代码块")}>
      <FieldGrid>
        <SettingsField label={t("Font size", "代码字体大小")} anchor="appearance.markdown.codeBlock.fontSize">
          <ChoicePills
            value={preferences.codeBlock.fontSize}
            options={[
              { value: "small", label: t("Small", "较小") },
              { value: "medium", label: t("Medium", "标准") },
              { value: "large", label: t("Large", "较大") }
            ]}
            ariaLabel={t("Code font size", "代码字体大小")}
            onChange={(fontSize) => onCodeBlockChange({ fontSize })}
          />
        </SettingsField>
        <SettingsField label={t("Tab width", "制表符宽度")} anchor="appearance.markdown.codeBlock.tabSize">
          <ChoicePills
            value={preferences.codeBlock.tabSize}
            options={[
              { value: "2", label: t("2 spaces", "2 个空格") },
              { value: "4", label: t("4 spaces", "4 个空格") },
              { value: "8", label: t("8 spaces", "8 个空格") }
            ]}
            ariaLabel={t("Tab width", "制表符宽度")}
            onChange={(tabSize) => onCodeBlockChange({ tabSize })}
          />
        </SettingsField>
        <SettingsField label={t("Maximum height", "最大高度")} anchor="appearance.markdown.codeBlock.maxHeight">
          <ChoicePills
            value={preferences.codeBlock.maxHeight}
            options={[
              { value: "none", label: t("No limit", "不限制") },
              { value: "medium", label: t("Medium · 24 rem", "中等 · 24rem") },
              { value: "tall", label: t("Tall · 36 rem", "较高 · 36rem") }
            ]}
            ariaLabel={t("Code block maximum height", "代码块最大高度")}
            onChange={(maxHeight) => onCodeBlockChange({ maxHeight })}
          />
        </SettingsField>
        <SwitchField
          label={t("Line numbers", "显示行号")}
          hint={t("Show a fixed number column for every source line.", "为每一行源码显示连续编号。")}
          checked={preferences.codeBlock.lineNumbers}
          anchor="appearance.markdown.codeBlock.lineNumbers"
          onChange={(lineNumbers) => onCodeBlockChange({ lineNumbers })}
        />
        <SwitchField
          label={t("Wrap long lines", "长行换行")}
          hint={t("Wrap long source lines instead of scrolling horizontally.", "长源码行自动折行，不使用横向滚动。")}
          checked={preferences.codeBlock.wrapLongLines}
          anchor="appearance.markdown.codeBlock.wrapLongLines"
          onChange={(wrapLongLines) => onCodeBlockChange({ wrapLongLines })}
        />
        <SwitchField
          label={t("Language label", "语言标签")}
          hint={t("Show the detected language in the code block header.", "在代码块头部显示识别到的语言。")}
          checked={preferences.codeBlock.showLanguageLabel}
          anchor="appearance.markdown.codeBlock.showLanguageLabel"
          onChange={(showLanguageLabel) => onCodeBlockChange({ showLanguageLabel })}
        />
        <SwitchField
          label={t("Copy button", "复制按钮")}
          hint={t("Show the copy action in the code block header.", "在代码块头部显示复制操作。")}
          checked={preferences.codeBlock.showCopyButton}
          anchor="appearance.markdown.codeBlock.showCopyButton"
          onChange={(showCopyButton) => onCodeBlockChange({ showCopyButton })}
        />
        <SwitchField
          label={t("Block border", "代码块外框")}
          hint={t("Add a thin neutral border around code blocks.", "为代码块增加同色系细边框。")}
          checked={preferences.codeBlock.showBorder}
          anchor="appearance.markdown.codeBlock.showBorder"
          onChange={(showBorder) => onCodeBlockChange({ showBorder })}
        />
      </FieldGrid>
    </SettingsPanel>
  );
}
