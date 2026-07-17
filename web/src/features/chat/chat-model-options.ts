import type { AppConfig, RunModelSelection } from "../../api/contracts";

export type ChatModelChoice = RunModelSelection & {
  providerName: string;
};

/**
 * 把应用配置转换为输入区可选择的模型列表。
 *
 * @param config Sai 应用配置
 * @returns 去重后的供应商模型选项
 */
export function buildChatModelChoices(config: AppConfig): ChatModelChoice[] {
  const seen = new Set<string>();
  return config.providers.flatMap((provider) => {
    const models = provider.models?.length ? provider.models : [provider.default_model ?? ""];
    return models.flatMap((model) => {
      const normalized = model.trim();
      const key = `${provider.id}\u0000${normalized}`;
      if (!normalized || seen.has(key)) return [];
      seen.add(key);
      return [{ providerId: provider.id, providerName: provider.display_name || provider.id, model: normalized }];
    });
  });
}

/**
 * 从用户偏好和应用默认值中解析当前模型。
 *
 * @param config Sai 应用配置
 * @param preferred 本地保存的模型偏好
 * @returns 当前有效模型，未配置模型时返回空值
 */
export function resolveChatModelSelection(
  config: AppConfig,
  preferred: RunModelSelection | null
): ChatModelChoice | null {
  const choices = buildChatModelChoices(config);
  const preferredChoice = choices.find((choice) => (
    choice.providerId === preferred?.providerId && choice.model === preferred.model
  ));
  if (preferredChoice) return preferredChoice;
  const activeProvider = config.providers.find((provider) => provider.id === config.active_provider);
  const activeModel = activeProvider?.default_model;
  return choices.find((choice) => choice.providerId === config.active_provider && choice.model === activeModel)
    ?? choices.find((choice) => choice.providerId === config.active_provider)
    ?? choices[0]
    ?? null;
}
