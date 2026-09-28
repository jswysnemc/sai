import type { AppConfig } from "../../../api/contracts";
import { SettingsField, SkSelect } from "../kit";
import { AGENT_THINKING_OPTIONS, buildAgentModelChoices } from "../../agents/agent-runtime-options";
import { useI18n } from "../../i18n/use-i18n";

type AgentRuntimePatch = {
  provider_id?: string;
  model?: string;
  thinking_level?: string;
};

type AgentRuntimeFieldsProps = {
  /** 应用配置 */
  config: AppConfig;
  /** 当前独立供应商标识 */
  providerId: string;
  /** 当前独立模型 */
  model: string;
  /** 当前思考等级 */
  thinkingLevel: string;
  /** 空模型选项文案 */
  inheritModelLabel: string;
  /** 思考等级字段说明 */
  thinkingHelp: string;
  /** 运行参数变化回调 */
  onChange: (patch: AgentRuntimePatch) => void;
};

/**
 * 渲染统一 Agent 使用的模型组合和思考等级字段。
 *
 * @param props 运行覆盖值、继承规则、字段文案和更新回调
 * @returns 两个运行参数表单字段
 */
export function AgentRuntimeFields({
  config,
  providerId,
  model,
  thinkingLevel,
  inheritModelLabel,
  thinkingHelp,
  onChange
}: AgentRuntimeFieldsProps) {
  const { t } = useI18n();
  const modelChoices = buildAgentModelChoices(config, providerId, model);
  const current = providerId && model ? `${providerId}\t${model}` : "";

  return <>
    <SettingsField label={t("Model", "模型")} anchor="agents.model" hint={t("Task-specific model settings take priority over this profile; this profile overrides shared defaults.", "任务专属模型优先于此档案，此档案优先于共享默认值。")}>
      <SkSelect
        value={current}
        options={[{ value: "", label: inheritModelLabel }, ...modelChoices]}
        onChange={(value) => {
          const [nextProvider = "", nextModel = ""] = value.split("\t");
          onChange({ provider_id: nextProvider, model: nextModel });
        }}
        disabled={modelChoices.length === 0}
        ariaLabel={t("Agent model", "Agent 模型")}
      />
    </SettingsField>
    <SettingsField label={t("Thinking level", "思考等级")} anchor="agents.thinking_level" hint={thinkingHelp}>
      <SkSelect value={thinkingLevel || "auto"} options={AGENT_THINKING_OPTIONS} onChange={(value) => onChange({ thinking_level: value })} ariaLabel={t("Agent thinking level", "Agent 思考等级")} />
    </SettingsField>
  </>;
}
