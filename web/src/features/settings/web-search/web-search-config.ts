import type { AppConfig } from "../../../api/contracts";

export type WebSearchProviderId = "tinyfish" | "tavily" | "firecrawl" | "anysearch" | "searxng" | "duckduckgo";
export type WebSearchDefaultProvider = "auto" | WebSearchProviderId;

/** Web 搜索总配置与供应商详细参数。 */
export type WebSearchConfig = {
  enabled: boolean;
  default_provider: WebSearchDefaultProvider;
  max_results: number;
  timeout_seconds: number;
  tinyfish_enabled: boolean;
  tinyfish_api_keys: string[];
  tinyfish_base_url: string;
  tinyfish_default_location: string;
  tinyfish_default_language: string;
  tavily_enabled: boolean;
  tavily_api_keys: string[];
  tavily_base_url: string;
  tavily_search_depth: "basic" | "advanced";
  tavily_include_answer: boolean;
  tavily_include_raw_content: boolean;
  firecrawl_enabled: boolean;
  firecrawl_api_keys: string[];
  firecrawl_base_url: string;
  firecrawl_only_main_content: boolean;
  anysearch_enabled: boolean;
  anysearch_api_keys: string[];
  anysearch_base_url: string;
  searxng_enabled: boolean;
  searxng_base_url: string;
  searxng_language: string;
  searxng_safe_search: number;
  duckduckgo_enabled: boolean;
  [key: string]: unknown;
};

export const WEB_SEARCH_PROVIDER_IDS: WebSearchProviderId[] = [
  "tinyfish",
  "tavily",
  "firecrawl",
  "anysearch",
  "searxng",
  "duckduckgo"
];

export const DEFAULT_WEB_SEARCH_CONFIG: WebSearchConfig = {
  enabled: true,
  default_provider: "auto",
  max_results: 5,
  timeout_seconds: 20,
  tinyfish_enabled: true,
  tinyfish_api_keys: [],
  tinyfish_base_url: "https://api.search.tinyfish.ai",
  tinyfish_default_location: "",
  tinyfish_default_language: "",
  tavily_enabled: true,
  tavily_api_keys: [],
  tavily_base_url: "https://api.tavily.com/search",
  tavily_search_depth: "basic",
  tavily_include_answer: false,
  tavily_include_raw_content: true,
  firecrawl_enabled: true,
  firecrawl_api_keys: [],
  firecrawl_base_url: "https://api.firecrawl.dev/v2/search",
  firecrawl_only_main_content: true,
  anysearch_enabled: true,
  anysearch_api_keys: [],
  anysearch_base_url: "https://api.anysearch.com/v1/search",
  searxng_enabled: true,
  searxng_base_url: "",
  searxng_language: "auto",
  searxng_safe_search: 0,
  duckduckgo_enabled: true
};

/**
 * 从通用 AppConfig 读取并补齐 Web 搜索配置。
 *
 * @param config 当前应用配置
 * @returns 可直接供表单使用的完整 Web 搜索配置
 */
export function readWebSearchConfig(config: AppConfig): WebSearchConfig {
  const raw = (config.plugins?.web ?? {}) as Record<string, unknown>;
  return {
    ...raw,
    enabled: booleanValue(raw.enabled, DEFAULT_WEB_SEARCH_CONFIG.enabled),
    default_provider: providerValue(raw.default_provider),
    max_results: numberValue(raw.max_results, DEFAULT_WEB_SEARCH_CONFIG.max_results),
    timeout_seconds: numberValue(raw.timeout_seconds, DEFAULT_WEB_SEARCH_CONFIG.timeout_seconds),
    tinyfish_enabled: booleanValue(raw.tinyfish_enabled, true),
    tinyfish_api_keys: stringArrayValue(raw.tinyfish_api_keys),
    tinyfish_base_url: stringValue(raw.tinyfish_base_url, DEFAULT_WEB_SEARCH_CONFIG.tinyfish_base_url),
    tinyfish_default_location: stringValue(raw.tinyfish_default_location, ""),
    tinyfish_default_language: stringValue(raw.tinyfish_default_language, ""),
    tavily_enabled: booleanValue(raw.tavily_enabled, true),
    tavily_api_keys: stringArrayValue(raw.tavily_api_keys),
    tavily_base_url: stringValue(raw.tavily_base_url, DEFAULT_WEB_SEARCH_CONFIG.tavily_base_url),
    tavily_search_depth: raw.tavily_search_depth === "advanced" ? "advanced" : "basic",
    tavily_include_answer: booleanValue(raw.tavily_include_answer, false),
    tavily_include_raw_content: booleanValue(raw.tavily_include_raw_content, true),
    firecrawl_enabled: booleanValue(raw.firecrawl_enabled, true),
    firecrawl_api_keys: stringArrayValue(raw.firecrawl_api_keys),
    firecrawl_base_url: stringValue(raw.firecrawl_base_url, DEFAULT_WEB_SEARCH_CONFIG.firecrawl_base_url),
    firecrawl_only_main_content: booleanValue(raw.firecrawl_only_main_content, true),
    anysearch_enabled: booleanValue(raw.anysearch_enabled, true),
    anysearch_api_keys: stringArrayValue(raw.anysearch_api_keys),
    anysearch_base_url: stringValue(raw.anysearch_base_url, DEFAULT_WEB_SEARCH_CONFIG.anysearch_base_url),
    searxng_enabled: booleanValue(raw.searxng_enabled, true),
    searxng_base_url: stringValue(raw.searxng_base_url, ""),
    searxng_language: stringValue(raw.searxng_language, "auto"),
    searxng_safe_search: numberValue(raw.searxng_safe_search, 0),
    duckduckgo_enabled: booleanValue(raw.duckduckgo_enabled, true)
  };
}

/**
 * 将完整 Web 搜索配置写回 AppConfig，同时保留其他 CLI 工具配置。
 *
 * @param config 当前应用配置
 * @param webSearch 新 Web 搜索配置
 * @returns 更新后的应用配置
 */
export function writeWebSearchConfig(config: AppConfig, webSearch: WebSearchConfig): AppConfig {
  return {
    ...config,
    plugins: {
      ...(config.plugins ?? {}),
      web: { ...(config.plugins?.web ?? {}), ...webSearch }
    }
  };
}

/**
 * 在默认供应商被关闭时回退到自动选择。
 *
 * @param config 待校正的 Web 搜索配置
 * @returns 默认供应商有效的配置
 */
export function normalizeWebSearchSelection(config: WebSearchConfig): WebSearchConfig {
  if (config.default_provider === "auto" || webSearchProviderAvailable(config, config.default_provider)) return config;
  return { ...config, default_provider: "auto" };
}

/**
 * 判断指定搜索供应商是否启用。
 *
 * @param config Web 搜索配置
 * @param provider 供应商标识
 * @returns 启用时返回 true
 */
export function webSearchProviderEnabled(config: WebSearchConfig, provider: WebSearchProviderId): boolean {
  return Boolean(config[`${provider}_enabled`]);
}

/**
 * 判断供应商是否可作为默认路由目标。
 *
 * @param config Web 搜索配置
 * @param provider 供应商标识
 * @returns 供应商启用且具备必要连接信息时返回 true
 */
export function webSearchProviderAvailable(
  config: WebSearchConfig,
  provider: WebSearchProviderId
): boolean {
  if (!webSearchProviderEnabled(config, provider)) return false;
  return provider !== "searxng" || Boolean(config.searxng_base_url.trim());
}

/**
 * 判断供应商是否具备执行搜索所需的最小配置。
 *
 * @param config Web 搜索配置
 * @param provider 供应商标识
 * @returns 已配置、内置或缺少配置三种状态
 */
export function webSearchProviderStatus(config: WebSearchConfig, provider: WebSearchProviderId): "configured" | "builtin" | "missing" {
  if (provider === "duckduckgo") return "builtin";
  if (provider === "searxng") return config.searxng_base_url.trim() ? "configured" : "missing";
  const keys = config[`${provider}_api_keys`];
  return Array.isArray(keys) && keys.some((key) => typeof key === "string" && key.trim()) ? "configured" : "missing";
}

/**
 * 合并界面可编辑密钥与服务端隐藏密钥占位符。
 *
 * @param current 当前密钥列表
 * @param text 用户输入的多行密钥
 * @param secretSentinel 服务端敏感字段占位符
 * @returns 保留隐藏密钥并追加可见输入的密钥列表
 */
export function mergeSearchApiKeyText(current: string[], text: string, secretSentinel: string): string[] {
  const visible = text.split(/[\n\r,]/).map((key) => key.trim()).filter(Boolean);
  if (!secretSentinel) return visible;

  // 1. 保持隐藏占位符的数组位置，服务端才能按索引恢复对应密钥
  let visibleIndex = 0;
  const merged = current.map((key) => {
    if (key === secretSentinel) return key;
    const replacement = visible[visibleIndex];
    visibleIndex += 1;
    return replacement ?? "";
  });
  // 2. 新增条目追加到现有位置之后
  merged.push(...visible.slice(visibleIndex));
  // 3. 移除末尾无占位意义的空槽，隐藏密钥之前的空槽必须保留
  while (merged.at(-1) === "") merged.pop();
  return merged;
}

/** 返回界面允许直接编辑的密钥或环境变量引用。 */
export function visibleSearchApiKeys(keys: string[], secretSentinel: string): string[] {
  return secretSentinel ? keys.filter((key) => key !== secretSentinel) : keys;
}

/** 返回服务端已隐藏的密钥数量。 */
export function hiddenSearchApiKeyCount(keys: string[], secretSentinel: string): number {
  return secretSentinel ? keys.filter((key) => key === secretSentinel).length : 0;
}

/** 将未知值转换为布尔配置。 */
function booleanValue(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

/** 将未知值转换为数值配置。 */
function numberValue(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

/** 将未知值转换为字符串配置。 */
function stringValue(value: unknown, fallback: string): string {
  return typeof value === "string" ? value : fallback;
}

/** 将未知值转换为字符串数组配置。 */
function stringArrayValue(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

/** 将未知值转换为合法默认供应商。 */
function providerValue(value: unknown): WebSearchDefaultProvider {
  return value === "auto" || WEB_SEARCH_PROVIDER_IDS.includes(value as WebSearchProviderId)
    ? value as WebSearchDefaultProvider
    : "auto";
}
