import type { AppConfig } from "../../../api/contracts";

/** 完整配置文档的解析结果。 */
export type SettingsJsonResult = { config: AppConfig; error: null } | { config: null; error: string };

/**
 * 【Web 设置】【JSON 校验】解析完整配置并拒绝非对象根节点。
 * @param text 配置文档文本
 * @returns 合法配置或解析错误
 */
export function parseSettingsJson(text: string): SettingsJsonResult {
  try {
    const config: unknown = JSON.parse(text);
    if (!config || typeof config !== "object" || Array.isArray(config)) return { config: null, error: "Configuration must be a JSON object" };
    return { config: config as AppConfig, error: null };
  } catch (error) {
    return { config: null, error: error instanceof Error ? error.message : String(error) };
  }
}
