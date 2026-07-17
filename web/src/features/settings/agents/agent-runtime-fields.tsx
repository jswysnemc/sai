import type { AppConfig } from "../../../api/contracts";
import { ModelIcon } from "../../../shared/ui/model-icon";
import { Select, type SelectOption } from "../../../shared/ui/select/select";
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

const THINKING_OPTIONS = ["auto", "none", "low", "medium", "high", "xhigh", "max"]
  .map((value) => ({ value, label: value }));

/**
 * 构造当前运行覆盖可选择的模型名称，并保留模型列表外的历史配置值。
 *
 * @param config 应用配置
 * @param providerId 当前独立供应商标识
 * @param currentModel 当前模型覆盖值
 * @returns 可直接用于单一模型选择器的选项
 */
export function buildAgentModelChoices(config: AppConfig, providerId: string, currentModel: string) {
  const choices: SelectOption<string>[] = config.providers.flatMap((provider) => {
    const configured = provider.models ?? [];
    const models = configured.length > 0
      ? configured
      : [provider.default_model].filter((model): model is string => Boolean(model));
    return models.map((model) => ({
      value: `${provider.id}\t${model}`,
      label: `${provider.display_name || provider.id} / ${model}`,
      icon: <ModelIcon model={model} size={14} />
    }));
  });
  if (providerId && currentModel && !choices.some((choice) => choice.value === `${providerId}\t${currentModel}`)) {
    choices.unshift({
      value: `${providerId}\t${currentModel}`,
      label: `${providerId} / ${currentModel}`,
      icon: <ModelIcon model={currentModel} size={14} />
    });
  }
  return choices;
}

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
    <div className="settings-field">
      <span>{t("Model", "模型")}</span>
      <Select
        value={current}
        options={[{ value: "", label: inheritModelLabel }, ...modelChoices]}
        onChange={(value) => {
          const [nextProvider = "", nextModel = ""] = value.split("\t");
          onChange({ provider_id: nextProvider, model: nextModel });
        }}
        disabled={modelChoices.length === 0}
        ariaLabel={t("Agent model", "Agent 模型")}
      />
      <small>{t("Select an enabled provider and model combination", "直接选择已启用的供应商与模型组合")}</small>
    </div>
    <div className="settings-field">
      <span>{t("Thinking level", "思考等级")}</span>
      <Select value={thinkingLevel || "auto"} options={THINKING_OPTIONS} onChange={(value) => onChange({ thinking_level: value })} ariaLabel={t("Agent thinking level", "Agent 思考等级")} />
      <small>{thinkingHelp}</small>
    </div>
  </>;
}
