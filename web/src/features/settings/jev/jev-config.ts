import type { AppConfig, ModelEndpointConfig } from "../../../api/contracts";
import type { JevAuditConfig, JevConfig, JevRoutingConfig } from "../../../api/contracts/jev";

/** 与后端 JevConfig 默认值保持一致。 */
export const DEFAULT_JEV_CONFIG: JevConfig = {
  endpoint_id: "",
  routing: { enabled: false, threshold: 0.5, max_tools: 6, max_skills: 3, timeout_seconds: 20, context_chars: 2000, prompt_segments: true },
  audit: { enabled: false, minimum_probability: 0.9, minimum_confidence: 0.8, timeout_seconds: 10 }
};

/** TypeSafe 官方 systemone 地址，新建 JEV 接入时预填。 */
export const JEV_OFFICIAL_ENDPOINT = "https://api.typesafe.ai/v1/systemone";

/**
 * 读取配置中的 Jev 段，缺省字段补齐默认值。
 * @param config 应用配置
 * @returns 完整的 Jev 配置
 */
export function readJevConfig(config: AppConfig): JevConfig {
  const jev = config.jev;
  return {
    endpoint_id: jev?.endpoint_id ?? "",
    routing: { ...DEFAULT_JEV_CONFIG.routing, ...jev?.routing },
    audit: { ...DEFAULT_JEV_CONFIG.audit, ...jev?.audit }
  };
}

/**
 * 更新暴露决策字段。
 * @param config 应用配置
 * @param patch 字段补丁
 * @returns 新配置
 */
export function patchJevRouting(config: AppConfig, patch: Partial<JevRoutingConfig>): AppConfig {
  const jev = readJevConfig(config);
  return { ...config, jev: { ...jev, routing: { ...jev.routing, ...patch } } };
}

/**
 * 更新权限审核字段。
 * @param config 应用配置
 * @param patch 字段补丁
 * @returns 新配置
 */
export function patchJevAudit(config: AppConfig, patch: Partial<JevAuditConfig>): AppConfig {
  const jev = readJevConfig(config);
  return { ...config, jev: { ...jev, audit: { ...jev.audit, ...patch } } };
}

type MemorySection = { jev_injection?: boolean; [key: string]: unknown };

/**
 * 读取记忆按需注入开关；顶层 memory 镜像优先，与后端生效规则一致。
 * @param config 应用配置
 * @returns 是否开启
 */
export function readJevMemoryInjection(config: AppConfig): boolean {
  const root = config.memory as MemorySection | undefined;
  const plugin = config.plugins?.memory as MemorySection | undefined;
  return root?.jev_injection ?? plugin?.jev_injection ?? false;
}

/**
 * 写入记忆按需注入开关：持久化到 plugins.memory，已有顶层镜像时同步更新。
 * @param config 应用配置
 * @param enabled 是否开启
 * @returns 新配置
 */
export function patchJevMemoryInjection(config: AppConfig, enabled: boolean): AppConfig {
  const plugins = config.plugins ?? {};
  const plugin = (plugins.memory as MemorySection | undefined) ?? {};
  const root = config.memory as MemorySection | undefined;
  return {
    ...config,
    plugins: { ...plugins, memory: { ...plugin, jev_injection: enabled } },
    ...(root ? { memory: { ...root, jev_injection: enabled } } : {})
  };
}

/**
 * 选择生效的 JEV 接入；空字符串表示自动。
 * @param config 应用配置
 * @param endpointId 接入 id
 * @returns 新配置
 */
export function selectJevEndpoint(config: AppConfig, endpointId: string): AppConfig {
  return { ...config, jev: { ...readJevConfig(config), endpoint_id: endpointId } };
}

/**
 * 列出全部 JEV 接入。
 * @param config 应用配置
 * @returns 类型为 jev 的接入
 */
export function jevEndpoints(config: AppConfig): ModelEndpointConfig[] {
  return (config.model_endpoints ?? []).filter((item) => item.kind === "jev");
}

/**
 * 删除接入时同步清除指向它的 Jev 引用，避免保存时校验失败。
 * @param config 已移除接入的配置
 * @param removedId 被删除的接入 id
 * @returns 新配置
 */
export function releaseJevEndpoint(config: AppConfig, removedId: string): AppConfig {
  if (config.jev?.endpoint_id !== removedId) return config;
  return selectJevEndpoint(config, "");
}

/**
 * 把输入框文本收敛为范围内的数值，空输入或非数字时回到下限。
 * @param raw 输入框文本
 * @param min 下限
 * @param max 上限
 * @param integer 是否取整
 * @returns 范围内的数值
 */
export function clampNumber(raw: string, min: number, max: number, integer = false): number {
  const parsed = Number(raw);
  if (!Number.isFinite(parsed) || raw.trim() === "") return min;
  const value = integer ? Math.round(parsed) : parsed;
  return Math.min(max, Math.max(min, value));
}
