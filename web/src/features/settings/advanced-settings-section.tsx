import { EditorHeader } from "./editor-layout";
import { JsonCodeEditor } from "../../shared/ui/code-editor/json-code-editor";
import { useI18n } from "../i18n/use-i18n";

type AdvancedSettingsSectionProps = {
  value: string;
  onChange: (value: string) => void;
};

/**
 * 渲染完整 AppConfig JSON 编辑器，编辑器占满内容区宽度。
 *
 * @param props JSON 文本和更新回调
 * @returns 高级设置区域
 */
export function AdvancedSettingsSection({ value, onChange }: AdvancedSettingsSectionProps) {
  const { t } = useI18n();
  return (
    <section className="settings-editor advanced-settings">
      <EditorHeader kicker={t("Complete configuration", "完整配置")} title={t("Advanced JSON", "高级 JSON")} description={t("When saving, the server deserializes the configuration again, merges sensitive fields, and performs full validation.", "保存时服务端会重新反序列化、合并敏感字段并执行完整校验。")} />
      <div className="advanced-settings-note">{t("Edit tool, display, prompt, and plugin options not yet covered by structured settings here.", "结构化设置尚未覆盖的工具、显示、提示词和插件选项可在此修改。")}</div>
      <JsonCodeEditor value={value} onChange={onChange} height="calc(100vh - 230px)" ariaLabel={t("Complete AppConfig JSON", "完整 AppConfig JSON")} />
    </section>
  );
}
