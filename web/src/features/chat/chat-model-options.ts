import type { AppConfig, RunModelSelection } from "../../api/contracts";
import { enabledProviders } from "../settings/model/provider-enablement";

export type ChatModelChoice = RunModelSelection & {
  providerName: string;
};

export type ChatModelGroup = {
  providerId: string;
  providerName: string;
  models: ChatModelChoice[];
};

/**
 * 把应用配置转换为输入区可选择的模型列表。
 *
 * 已停用的供应商整体跳过：它的模型不能被选中，否则停用开关形同虚设。
 *
 * @param config Sai 应用配置
 * @returns 去重后的供应商模型选项
 */
export function buildChatModelChoices(config: AppConfig): ChatModelChoice[] {
  const seen = new Set<string>();
  return enabledProviders(config.providers).flatMap((provider) => {
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
 * 输入框里展示的短模型名：去掉 `路由/分组/` 前缀，只保留最后一段。
 *
 * 选择菜单仍显示完整 ID，便于区分同名模型的不同来源。
 *
 * @param model 完整模型 ID，如 `clinepass/cline-pass/deepseek-v4-flash`
 * @returns 最后一段模型名；无斜杠或末段为空时返回原值
 */
export function shortModelName(model: string): string {
  const last = model.trim().split("/").filter(Boolean).at(-1);
  return last ?? model;
}

/**
 * 按供应商把可选模型分成二级分组。
 *
 * @param choices 扁平模型选项
 * @returns 按供应商顺序排列的分组
 */
export function groupChatModelChoices(choices: ChatModelChoice[]): ChatModelGroup[] {
  const groups: ChatModelGroup[] = [];
  const index = new Map<string, number>();
  for (const choice of choices) {
    const existing = index.get(choice.providerId);
    if (existing === undefined) {
      index.set(choice.providerId, groups.length);
      groups.push({ providerId: choice.providerId, providerName: choice.providerName, models: [choice] });
      continue;
    }
    groups[existing].models.push(choice);
  }
  return groups;
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
