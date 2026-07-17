import { EditorHeader } from "./editor-layout";
import { JsonCodeEditor } from "../../shared/ui/code-editor/json-code-editor";

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
  return (
    <section className="settings-editor advanced-settings">
      <EditorHeader kicker="完整配置" title="高级 JSON" description="保存时服务端会重新反序列化、合并敏感字段并执行完整校验。" />
      <div className="advanced-settings-note">结构化设置尚未覆盖的工具、显示、提示词和插件选项可在此修改。</div>
      <JsonCodeEditor value={value} onChange={onChange} height="calc(100vh - 230px)" ariaLabel="完整 AppConfig JSON" />
    </section>
  );
}
