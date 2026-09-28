import type { ProviderConfig } from "../../../api/contracts";

/** 远端目录可补充的模型元数据。 */
export type ImportedModelMetadata = {
  context_chars?: number | null;
  max_output_tokens?: number | null;
  tags?: string[];
  thinking_levels?: string[];
};

/**
 * 【供应商设置】【模型导入】合并选中模型，保留手工填写的限制与推理等级。
 * @param provider 当前供应商
 * @param selected 本次勾选的模型标识
 * @param remote 远端目录元数据
 * @returns 仅包含模型目录和默认模型的更新补丁
 */
export function mergeProviderModels(provider: ProviderConfig, selected: readonly string[], remote: Record<string, ImportedModelMetadata>): Partial<ProviderConfig> {
  const models = [...new Set([...(provider.models ?? []), ...selected])];
  const metadata = { ...provider.model_metadata };
  for (const model of selected) {
    const imported = remote[model];
    if (!imported?.context_chars && !imported?.max_output_tokens && !imported?.tags?.length && !imported?.thinking_levels?.length) continue;
    const current = metadata[model] ?? {};
    metadata[model] = {
      ...current,
      ...(!current.context_chars && imported.context_chars ? { context_chars: imported.context_chars } : {}),
      ...(!current.max_output_tokens && imported.max_output_tokens ? { max_output_tokens: imported.max_output_tokens } : {}),
      ...(imported.tags?.length ? { tags: [...new Set([...(current.tags ?? []), ...imported.tags])] } : {}),
      ...(!current.thinking_levels?.length && imported.thinking_levels?.length ? { thinking_levels: imported.thinking_levels } : {})
    };
  }
  return { models, model_metadata: metadata, default_model: provider.default_model || models[0] || "" };
}
