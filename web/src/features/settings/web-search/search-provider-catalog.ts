import {
  Compass,
  Flame,
  Globe2,
  Network,
  Radar,
  Search,
  ShieldCheck,
  Sparkles
} from "../../../shared/ui/icons";
import type { LucideIcon } from "../../../shared/ui/icons";
import type { Locale } from "../../i18n/locale";
import type { WebSearchProviderId } from "./web-search-config";

export type SearchProviderCatalogEntry = {
  id: WebSearchProviderId;
  label: string;
  descriptionEn: string;
  descriptionZh: string;
  environmentVariable?: string;
  icon: LucideIcon;
  features: Array<[string, string]>;
};

export const SEARCH_PROVIDER_CATALOG: SearchProviderCatalogEntry[] = [
  {
    id: "tinyfish",
    features: [["Location", "位置偏好"], ["Language", "语言偏好"]],
    label: "TinyFish",
    descriptionEn: "Search API with location and language preferences",
    descriptionZh: "支持位置与语言偏好的搜索接口",
    environmentVariable: "TINYFISH_API_KEY",
    icon: Search
  },
  {
    id: "tavily",
    features: [["Deep search", "深入搜索"], ["Page content", "网页正文"], ["Answers", "综合答案"]],
    label: "Tavily",
    descriptionEn: "Research-oriented results with optional answers and raw content",
    descriptionZh: "面向研究的结果，可附带答案与原始正文",
    environmentVariable: "TAVILY_API_KEY",
    icon: Compass
  },
  {
    id: "firecrawl",
    features: [["Search", "搜索"], ["Content extraction", "正文提取"]],
    label: "Firecrawl",
    descriptionEn: "Search and extraction with main-content filtering",
    descriptionZh: "搜索与正文提取，可过滤页面非正文内容",
    environmentVariable: "FIRECRAWL_API_KEY",
    icon: Flame
  },
  {
    id: "anysearch",
    features: [["General search", "通用搜索"]],
    label: "AnySearch",
    descriptionEn: "General search API with a configurable endpoint",
    descriptionZh: "可配置服务地址的通用搜索接口",
    environmentVariable: "ANYSEARCH_API_KEY",
    icon: Radar
  },
  {
    id: "brave",
    features: [["Independent index", "独立索引"], ["Fresh results", "时效结果"]],
    label: "Brave",
    descriptionEn: "Independent web index with a subscription token",
    descriptionZh: "使用订阅令牌的独立网页索引",
    environmentVariable: "BRAVE_API_KEY",
    icon: ShieldCheck
  },
  {
    id: "exa",
    features: [["Neural search", "语义检索"], ["Page excerpts", "页面摘录"]],
    label: "Exa",
    descriptionEn: "Neural search API with optional page excerpts",
    descriptionZh: "带可选页面摘录的语义检索接口",
    environmentVariable: "EXA_API_KEY",
    icon: Sparkles
  },
  {
    id: "searxng",
    features: [["Metasearch", "聚合搜索"], ["Self-hosted", "自托管"]],
    label: "SearXNG",
    descriptionEn: "Self-hosted metasearch endpoint",
    descriptionZh: "自托管聚合搜索服务",
    icon: Network
  },
  {
    id: "duckduckgo",
    features: [["No API key", "无需密钥"], ["Fallback", "回退搜索"]],
    label: "DuckDuckGo",
    descriptionEn: "Built-in fallback without credentials",
    descriptionZh: "无需凭据的内置回退搜索",
    icon: Globe2
  }
];

/**
 * 按标识读取搜索供应商元数据。
 *
 * @param id 搜索供应商标识
 * @returns 对应供应商元数据
 */
export function getSearchProvider(id: WebSearchProviderId): SearchProviderCatalogEntry {
  return SEARCH_PROVIDER_CATALOG.find((provider) => provider.id === id) ?? SEARCH_PROVIDER_CATALOG[0];
}

/**
 * 返回当前语言下的供应商说明。
 *
 * @param provider 搜索供应商元数据
 * @param locale 当前界面语言
 * @returns 本地化供应商说明
 */
export function searchProviderDescription(
  provider: SearchProviderCatalogEntry,
  locale: Locale
): string {
  return locale === "zh-CN" ? provider.descriptionZh : provider.descriptionEn;
}

/**
 * 读取供应商对应的环境变量名称。
 *
 * @param id 搜索供应商标识
 * @returns 已登记的环境变量名称，未登记时返回空字符串
 */
export function searchProviderEnvironmentVariable(id: WebSearchProviderId): string {
  return getSearchProvider(id).environmentVariable ?? "";
}
