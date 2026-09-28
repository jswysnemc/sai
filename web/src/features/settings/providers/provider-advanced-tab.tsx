import { useState } from "react";
import { Button } from "../../../shared/ui/button/button";
import type { ProviderConfig } from "../../../api/contracts";
import { JsonCodeEditor, type JsonEditorDiagnostic } from "../../../shared/ui/code-editor/json-code-editor";
import { useI18n } from "../../i18n/use-i18n";
import { ChoicePills, FieldGrid, InlineNotice, SettingsField, SettingsPanel, SkTextInput, SwitchField } from "../kit";
import { KeyValueEditor } from "../key-value-editor";
import { isClaudeClientStyle, userAgentPlaceholder } from "./provider-options";

type ProviderAdvancedTabProps = { provider: ProviderConfig; onPatch: (patch: Partial<ProviderConfig>) => void };

/**
 * 【供应商设置】【高级请求】编辑客户端标识、请求头与自定义请求体。
 * @param props 供应商配置及局部更新回调
 * @returns 高级设置页签
 */
export function ProviderAdvancedTab({ provider, onPatch }: ProviderAdvancedTabProps) {
  const { t } = useI18n();
  const [diagnostics, setDiagnostics] = useState<JsonEditorDiagnostic[]>([]);
  return <>
    <SettingsPanel title={t("Client identity", "客户端标识")} description={t("Choose the request format and identity required by the upstream endpoint.", "选择上游接入要求的请求格式与客户端标识。")}>
      <FieldGrid>
        <SettingsField label={t("Client style", "客户端模拟")} anchor="providers.advanced.client_style" configKey="providers.client_style" hint={t("Codex uses Responses and CLI headers; Claude uses Anthropic Messages and Claude Code headers.", "Codex 使用 Responses 与 CLI 请求头；Claude 使用 Anthropic Messages 与 Claude Code 请求头。")}>
          <ChoicePills value={provider.client_style ?? "auto"} options={[{ value: "auto", label: t("Auto", "自动") }, { value: "default", label: t("Default", "默认") }, { value: "codex", label: "Codex CLI" }, { value: "claude", label: "Claude Code" }]} onChange={(value) => onPatch({ client_style: value })} />
        </SettingsField>
        <SettingsField label="User-Agent" anchor="providers.advanced.user_agent" configKey="providers.user_agent" hint={t("Empty uses the selected client's default. Overrides extra headers.", "留空使用所选客户端默认值，优先于自定义请求头。")}>
          <SkTextInput value={provider.user_agent ?? ""} placeholder={userAgentPlaceholder(provider)} onChange={(value) => onPatch({ user_agent: value })} />
        </SettingsField>
        {isClaudeClientStyle(provider.client_style) && <SwitchField label={t("Claude 1M context", "Claude 启用 1M 上下文")} anchor="providers.advanced.claude_1m_context" configKey="providers.claude_1m_context" hint={t("Add the extended-context beta header; enabled by default.", "附加扩展上下文请求头，默认启用。")} checked={provider.claude_1m_context !== false} onChange={(value) => onPatch({ claude_1m_context: value })} />}
      </FieldGrid>
    </SettingsPanel>
    <SettingsPanel title={t("Custom request payload", "自定义请求载荷")} description={t("Merged into each request; explicit fields take precedence.", "合并到每次模型请求，显式配置字段优先。")}>
      <FieldGrid columns={1}>
        <SettingsField label={t("Extra headers", "自定义请求头")} anchor="providers.advanced.extra_headers" configKey="providers.extra_headers" hint={t("Authorization is not overridden.", "不会覆盖 Authorization。")}>
          <div className="mb-2 flex flex-wrap gap-1" aria-label={t("Header presets", "请求头预设")}>
            {["OpenAI-Organization", "OpenAI-Project", "X-Custom"].map((header) => <Button size="small" key={header} disabled={Object.keys(provider.extra_headers ?? {}).some((key) => key.toLowerCase() === header.toLowerCase())} onClick={() => onPatch({ extra_headers: { ...provider.extra_headers, [header]: "" } })}>{header}</Button>)}
          </div>
          <KeyValueEditor value={provider.extra_headers ?? {}} onChange={(extra_headers) => onPatch({ extra_headers })} />
        </SettingsField>
        <SettingsField label={t("Custom body JSON", "自定义 body JSON")} anchor="providers.advanced.extra_body" configKey="providers.extra_body">
          {diagnostics[0] && <InlineNotice tone="danger">{t(`Line ${diagnostics[0].line}: ${diagnostics[0].message}`, `第 ${diagnostics[0].line} 行：${diagnostics[0].message}`)}</InlineNotice>}
          <JsonCodeEditor value={provider.extra_body || "{}"} onChange={(value) => onPatch({ extra_body: value === "{}" ? "" : value })} onDiagnostics={setDiagnostics} height="18rem" ariaLabel={t("Provider custom body JSON", "供应商自定义 body JSON")} />
        </SettingsField>
      </FieldGrid>
    </SettingsPanel>
  </>;
}
