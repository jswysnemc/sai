import type { AppConfig } from "../../api/contracts";
import { ModelIcon } from "../../shared/ui/model-icon";
import type { SelectOption } from "../../shared/ui/select/select";

export const AGENT_THINKING_OPTIONS = ["auto", "none", "low", "medium", "high", "xhigh", "max"]
  .map((value) => ({ value, label: value }));

/**
 * 构造 Agent 可选择的供应商与模型组合，并保留模型列表外的历史配置值。
 *
 * @param config 应用配置
 * @param providerId 当前独立供应商标识
 * @param currentModel 当前模型覆盖值
 * @returns 可直接用于模型选择器的选项
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
