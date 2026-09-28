import type { ProviderConfig } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { FieldGrid, SettingsField, SettingsPanel, SkNumberInput, SkSelect, SkTextInput, SwitchField } from "../kit";
import { isClaudeClientStyle, THINKING_OPTIONS, thinkingFormatOptions } from "./provider-options";

type ProviderBehaviorTabProps = {
  provider: ProviderConfig;
  temperatureDraft: string;
  temperatureError: string;
  onTemperatureDraftChange: (value: string) => void;
  onCommitTemperature: (value: string) => void;
  onPatch: (patch: Partial<ProviderConfig>) => void;
  onDeepseekAnchorChange: (enabled: boolean) => void;
};

/**
 * 【供应商设置】【请求行为】组合请求参数与当前模型专属行为。
 * @param props 供应商配置、温度草稿、校验状态与更新回调
 * @returns 行为设置页签
 */
export function ProviderBehaviorTab({ provider, temperatureDraft, temperatureError, onTemperatureDraftChange, onCommitTemperature, onPatch, onDeepseekAnchorChange }: ProviderBehaviorTabProps) {
  const { t } = useI18n();
  const model = provider.default_model ?? "";
  const anchored = provider.model_metadata?.[model]?.deepseek_anchor_mode === "anchored_standard";
  return <>
    <SettingsPanel title={t("Request parameters", "请求参数")} description={t("Defaults for requests sent to this provider.", "发往该供应商的请求默认使用这些参数。")}>
      <FieldGrid>
        <SettingsField label={t("Request timeout", "请求超时")} anchor="providers.behavior.timeout_seconds" configKey="providers.timeout_seconds" size="sm">
          <SkNumberInput value={provider.timeout_seconds ?? 120} min={1} integer unit={t("s", "秒")} onChange={(value) => onPatch({ timeout_seconds: value ?? 120 })} />
        </SettingsField>
        <SettingsField label={t("Temperature", "温度")} anchor="providers.behavior.temperature" configKey="providers.temperature" size="sm" error={temperatureError || undefined} hint={t("Optional value from 0 to 2. Empty uses the provider default.", "可选值为 0 到 2，留空使用供应商默认值。")}>
          <SkTextInput inputMode="decimal" value={temperatureDraft} onChange={onTemperatureDraftChange} onBlur={(event) => onCommitTemperature(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} />
        </SettingsField>
        <SettingsField label={t("Thinking level", "思考等级")} anchor="providers.behavior.thinking_level" configKey="providers.thinking_level">
          <SkSelect value={provider.thinking_level ?? "auto"} options={THINKING_OPTIONS} onChange={(value) => onPatch({ thinking_level: value })} />
        </SettingsField>
        <SettingsField label={t("Thinking format", "思考格式")} anchor="providers.behavior.thinking_format" configKey="providers.thinking_format" hint={t("Reasoning field used in responses.", "响应使用的思考字段格式。")}>
          <SkSelect value={provider.thinking_format ?? "auto"} options={thinkingFormatOptions()} onChange={(value) => onPatch({ thinking_format: value })} />
        </SettingsField>
        <SwitchField label={t("Preserve thinking", "回传历史思考")} anchor="providers.behavior.preserve_thinking" configKey="providers.preserve_thinking" hint={t("Send previous reasoning_content in multi-turn requests; some models require it.", "多轮请求回传历史 reasoning_content，部分模型要求启用此项。")} checked={provider.preserve_thinking === true} onChange={(value) => onPatch({ preserve_thinking: value })} />
      </FieldGrid>
    </SettingsPanel>
    <SettingsPanel title={t("Model-specific behavior", "模型特定行为")}>
      <FieldGrid>
        <SwitchField label={t("DeepSeek trajectory anchor", "DeepSeek 轨迹锚定")} anchor="providers.behavior.deepseek_anchor" hint={t("Use the dsh Anchored Standard tool flow for the default model.", "为默认模型启用 dsh Anchored Standard 工具流程。")} checked={anchored} disabled={!model} onChange={onDeepseekAnchorChange} />
        {isClaudeClientStyle(provider.client_style) && <SettingsField label={t("Claude max output", "Claude 最大输出")} anchor="providers.behavior.anthropic_max_tokens" configKey="providers.anthropic_max_tokens" size="sm">
          <SkNumberInput value={provider.anthropic_max_tokens ?? 8192} min={1} integer onChange={(value) => onPatch({ anthropic_max_tokens: value ?? 8192 })} />
        </SettingsField>}
      </FieldGrid>
    </SettingsPanel>
  </>;
}
