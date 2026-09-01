import type { ProviderConfig } from "../../../api/contracts";
import { Select } from "../../../shared/ui/select/select";
import { useI18n } from "../../i18n/use-i18n";
import { SettingsGroup } from "../editor-layout";
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
 * 供应商编辑器的行为页签：请求参数与模型特定行为。
 *
 * 按生效层次分两组：通用请求参数（超时/温度/思考）在前，
 * 针对特定模型的开关（DeepSeek 锚定、Claude 上限）在后。
 *
 * @param props 供应商状态与更新回调
 * @returns 行为页签内容
 */
export function ProviderBehaviorTab({
  provider,
  temperatureDraft,
  temperatureError,
  onTemperatureDraftChange,
  onCommitTemperature,
  onPatch,
  onDeepseekAnchorChange
}: ProviderBehaviorTabProps) {
  const { t } = useI18n();
  const activeModel = provider.default_model ?? "";
  const activeModelMetadata = activeModel ? (provider.model_metadata?.[activeModel] ?? {}) : {};
  const deepseekAnchorEnabled = activeModelMetadata.deepseek_anchor_mode === "anchored_standard";
  const claudeSimulation = isClaudeClientStyle(provider.client_style);

  return (
    <>
      <SettingsGroup
        title={t("Request parameters", "请求参数")}
        description={t(
          "Defaults applied to every request sent to this provider.",
          "发往该供应商的每次请求默认携带的参数。"
        )}
      >
        <div className="settings-form-grid">
          <label className="settings-field">
            <span>{t("Request timeout", "请求超时")}</span>
            <input
              type="number"
              min="1"
              value={provider.timeout_seconds ?? 120}
              onChange={(event) => onPatch({ timeout_seconds: Number(event.target.value) })}
            />
            <small>{t("Seconds", "单位为秒")}</small>
          </label>
          <label className="settings-field">
            <span>Temperature</span>
            <input
              type="text"
              inputMode="decimal"
              value={temperatureDraft}
              placeholder={t("Leave empty for provider default", "留空则不发送，由供应商默认")}
              onChange={(event) => onTemperatureDraftChange(event.target.value)}
              onBlur={(event) => onCommitTemperature(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") event.currentTarget.blur();
              }}
              aria-invalid={temperatureError ? true : undefined}
            />
            <small className={temperatureError ? "settings-field-error" : undefined}>{temperatureError || t("Optional sampling temperature from 0 to 2. Empty omits the field.", "可选采样温度，范围 0 到 2。留空则请求不带该参数。")}</small>
          </label>
          <div className="settings-field">
            <span>{t("Thinking level", "思考等级")}</span>
            <Select
              value={provider.thinking_level ?? "auto"}
              options={THINKING_OPTIONS}
              onChange={(value) => onPatch({ thinking_level: value })}
              ariaLabel={t("Thinking level", "思考等级")}
            />
            <small>{t("Default reasoning intensity for the provider", "供应商默认推理强度")}</small>
          </div>
          <div className="settings-field">
            <span>{t("Thinking format", "思考格式")}</span>
            <Select
              value={provider.thinking_format ?? "auto"}
              options={thinkingFormatOptions()}
              onChange={(value) => onPatch({ thinking_format: value })}
              ariaLabel={t("Thinking format", "思考格式")}
            />
            <small>{t("Reasoning field in the response", "响应中的思考字段")}</small>
          </div>
          <label className="settings-toggle-field settings-inline-toggle">
            <span>
              <strong>{t("Preserve thinking", "回传历史思考")}</strong>
              <small>{t(
                "Send previous reasoning_content back in multi-turn requests; required by models such as kimi-k2.7-code.",
                "多轮请求回传历史 reasoning_content；kimi-k2.7-code 一类模型要求开启。"
              )}</small>
            </span>
            <input
              type="checkbox"
              checked={provider.preserve_thinking === true}
              onChange={(event) => onPatch({ preserve_thinking: event.target.checked })}
            />
          </label>
        </div>
      </SettingsGroup>

      <SettingsGroup
        title={t("Model-specific behavior", "模型特定行为")}
        description={t(
          "Switches that only apply to particular models on this provider.",
          "只对该供应商上特定模型生效的开关。"
        )}
      >
        <div className="settings-form-grid">
          <label className="settings-toggle-field settings-inline-toggle">
            <span>
              <strong>{t("DeepSeek trajectory anchor", "DeepSeek 轨迹锚定")}</strong>
              <small>
                {t(
                  "Use the dsh Anchored Standard tool flow for the selected model's first request.",
                  "为当前默认模型启用 dsh Anchored Standard 首请求工具流程。"
                )}
              </small>
            </span>
            <input
              type="checkbox"
              checked={deepseekAnchorEnabled}
              disabled={!activeModel}
              onChange={(event) => onDeepseekAnchorChange(event.target.checked)}
            />
          </label>
          {claudeSimulation && (
            <label className="settings-field">
              <span>{t("Claude max output", "Claude 最大输出")}</span>
              <input
                type="number"
                min="1"
                value={provider.anthropic_max_tokens ?? 8192}
                onChange={(event) =>
                  onPatch({ anthropic_max_tokens: Number(event.target.value) })
                }
              />
              <small>{t("Anthropic Messages max_tokens for Claude simulation", "Claude 模拟时 Anthropic Messages 的 max_tokens")}</small>
            </label>
          )}
        </div>
      </SettingsGroup>
    </>
  );
}
