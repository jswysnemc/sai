import type { AppConfig, ModelEndpointConfig, ModelEndpointKind } from "../../../api/contracts/config";

/**
 * 【模型接入】【新增草稿】创建独立且稳定的标识，不改写聊天供应商。
 * @param endpoints 所有专用模型配置
 * @param kind 接入类型
 * @param name 初始显示名称
 * @returns 尚未填写请求地址的新配置
 */
export function newModelEndpoint(endpoints: readonly ModelEndpointConfig[], kind: ModelEndpointKind, name: string): ModelEndpointConfig {
  let index = 1;
  while (endpoints.some((item) => item.id === `${kind}-${index}`)) index += 1;
  return { id: `${kind}-${index}`, kind, name, endpoint: "", api_key: "", model: "" };
}

/**
 * 【模型接入】【编辑草稿】仅更新指定模型的连接字段，保留其他类型与普通供应商。
 * @param config 完整应用配置
 * @param id 目标接入标识
 * @param patch 连接字段补丁
 * @returns 新配置
 */
export function updateModelEndpoint(config: AppConfig, id: string, patch: Partial<Omit<ModelEndpointConfig, "id" | "kind">>): AppConfig {
  return { ...config, model_endpoints: (config.model_endpoints ?? []).map((item) => item.id === id ? { ...item, ...patch } : item) };
}
