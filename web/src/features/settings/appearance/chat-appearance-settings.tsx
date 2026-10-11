import { Button } from "../../../shared/ui/button/button";
import { MarkdownRenderer } from "../../chat/markdown-renderer";
import {
  getChatAppearanceStyle,
  useChatAppearance,
  type ChatFont,
  type ChatWidth
} from "../../chat/chat-appearance";
import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, SettingsField, SettingsPanel, SkNumberInput } from "../kit";
import "../appearance-settings-section.css";
import "../../chat/chat-appearance.css";

const FONT_OPTIONS: Array<{ value: ChatFont; labelEn: string; labelZh: string }> = [
  { value: "system", labelEn: "System", labelZh: "系统" },
  { value: "sans", labelEn: "Sans", labelZh: "无衬线" },
  { value: "serif", labelEn: "Serif", labelZh: "衬线" },
  { value: "mono", labelEn: "Mono", labelZh: "等宽" }
];

const WIDTH_OPTIONS: Array<{ value: ChatWidth; labelEn: string; labelZh: string }> = [
  { value: "standard", labelEn: "Standard", labelZh: "标准" },
  { value: "wide", labelEn: "Wide", labelZh: "宽栏" },
  { value: "full", labelEn: "Full", labelZh: "铺满" }
];

/**
 * 【Web 设置】【聊天阅读】调整对话列字体、字号与宽度，即时写到当前浏览器。
 * @returns 聊天阅读外观面板
 */
export function ChatAppearanceSettings() {
  const { t, locale } = useI18n();
  const { appearance, setAppearance, resetAppearance } = useChatAppearance();
  const preview = locale === "zh-CN"
    ? "### 更清晰的对话\n\n正文按阅读列排版，`const answer = 42` 保持等宽。\n\n| 列宽 | 用途 |\n| --- | --- |\n| 标准 | 50rem 阅读列 |\n| 宽栏 | 75rem |"
    : "### A clearer conversation\n\nReading text uses the conversation column. `const answer = 42` stays monospace.\n\n| Width | Use |\n| --- | --- |\n| Standard | 50rem measure |\n| Wide | 75rem |";

  return (
    <SettingsPanel
      title={t("Chat reading", "聊天阅读")}
      description={t(
        "Applies only to conversation text. Interface size stays under Text and zoom.",
        "只作用于对话正文。界面字号仍由「字号与缩放」控制。"
      )}
      actions={
        <Button size="small" onClick={resetAppearance}>
          {t("Reset", "恢复默认")}
        </Button>
      }
    >
      <SettingsField label={t("Chat font", "对话字体")} anchor="appearance.chat.font">
        <ChoicePills
          value={appearance.font}
          options={FONT_OPTIONS.map((option) => ({ value: option.value, label: t(option.labelEn, option.labelZh) }))}
          onChange={(font) => setAppearance({ font })}
          ariaLabel={t("Chat font", "对话字体")}
        />
      </SettingsField>
      <SettingsField
        label={t("Chat font size", "对话字号")}
        anchor="appearance.chat.fontSize"
        hint={t("12–24 px. Default 15px.", "12–24 像素，默认 15px。")}
        size="sm"
      >
        <SkNumberInput
          value={appearance.fontSize}
          min={12}
          max={24}
          integer
          step={1}
          unit="px"
          ariaLabel={t("Chat font size", "对话字号")}
          onChange={(value) => {
            if (value != null) setAppearance({ fontSize: value });
          }}
        />
      </SettingsField>
      <SettingsField label={t("Conversation width", "对话列宽")} anchor="appearance.chat.width">
        <ChoicePills
          value={appearance.width}
          options={WIDTH_OPTIONS.map((option) => ({ value: option.value, label: t(option.labelEn, option.labelZh) }))}
          onChange={(width) => setAppearance({ width })}
          ariaLabel={t("Conversation width", "对话列宽")}
        />
      </SettingsField>
      <section className="chat-appearance-preview" aria-label={t("Font preview", "字体预览")}>
        <header className="chat-appearance-preview-head">
          <strong>{t("Preview", "效果预览")}</strong>
        </header>
        <div className="chat-appearance-preview-body chat-reading-markdown" style={getChatAppearanceStyle(appearance)}>
          <MarkdownRenderer source={preview} />
        </div>
      </section>
    </SettingsPanel>
  );
}
