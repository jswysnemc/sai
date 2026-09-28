import { useState } from "react";
import { SettingsPanel, InlineNotice, StatusBadge } from "./kit";
import { Button } from "../../shared/ui/button/button";
import { JsonCodeEditor, type JsonEditorDiagnostic } from "../../shared/ui/code-editor/json-code-editor";
import { useI18n } from "../i18n/use-i18n";
import "./advanced-settings-section.css";

type AdvancedSettingsSectionProps = {
  raw: string;
  parseError: string | null;
  onChange: (text: string) => void;
};

/**
 * 【Web 设置】【完整文档】显示配置草稿、解析状态与错误定位入口。
 * @param props 控制器持有的文本草稿、解析错误及更新回调
 * @returns 完整配置编辑面板
 */
export function AdvancedSettingsSection({ raw, parseError, onChange }: AdvancedSettingsSectionProps) {
  const { t } = useI18n();
  const [diagnostics, setDiagnostics] = useState<JsonEditorDiagnostic[]>([]);
  const [reveal, setReveal] = useState<{ line: number; column: number; request: number }>();

  return (
    <SettingsPanel title={t("Configuration document", "配置文档")} className="advanced-settings" description={t("Edit settings not covered by the forms. The server validates the full document when saving.", "编辑表单尚未覆盖的配置项，保存时服务端会校验完整文档。")} actions={<StatusBadge tone={parseError ? "danger" : "success"}>{parseError ? t("Invalid JSON", "JSON 无效") : t("Valid JSON", "JSON 有效")}</StatusBadge>}>
      {parseError && <InlineNotice tone="danger" action={diagnostics[0] && <Button size="small" onClick={() => setReveal({ ...diagnostics[0], request: Date.now() })}>{t(`Go to line ${diagnostics[0].line}`, `定位第 ${diagnostics[0].line} 行`)}</Button>}>{parseError}</InlineNotice>}
      <div id="settings-field-advanced-json">
        <JsonCodeEditor value={raw} onChange={onChange} onDiagnostics={setDiagnostics} reveal={reveal} height="clamp(18rem, calc(100dvh - 13rem), 55rem)" ariaLabel={t("Complete AppConfig JSON", "完整 AppConfig JSON")} />
      </div>
    </SettingsPanel>
  );
}
