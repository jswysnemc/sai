import {
  Compass,
  Flame,
  Globe2,
  Network,
  Radar,
  Search
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { Locale } from "../../i18n/locale";
import type { WebSearchProviderId } from "./web-search-config";

export type SearchProviderCatalogEntry = {
  id: WebSearchProviderId;
  label: string;
  descriptionEn: string;
  descriptionZh: string;
  environmentVariable?: string;
  icon: LucideIcon;
};

export const SEARCH_PROVIDER_CATALOG: SearchProviderCatalogEntry[] = [
  {
    id: "tinyfish",
    label: "TinyFish",
    descriptionEn: "Search API with location and language preferences",
    descriptionZh: "支持位置与语言偏好的搜索接口",
    environmentVariable: "TINYFISH_API_KEY",
    icon: Search
  },
  {
    id: "tavily",
    label: "Tavily",
    descriptionEn: "Research-oriented results with optional answers and raw content",
    descriptionZh: "面向研究的结果，可附带答案与原始正文",
    environmentVariable: "TAVILY_API_KEY",
    icon: Compass
  },
  {
    id: "firecrawl",
    label: "Firecrawl",
    descriptionEn: "Search and extraction with main-content filtering",
    descriptionZh: "搜索与正文提取，可过滤页面非正文内容",
    environmentVariable: "FIRECRAWL_API_KEY",
    icon: Flame
  },
  {
    id: "anysearch",
    label: "AnySearch",
    descriptionEn: "General search API with a configurable endpoint",
    descriptionZh: "可配置服务地址的通用搜索接口",
    environmentVariable: "ANYSEARCH_API_KEY",
    icon: Radar
  },
  {
    id: "searxng",
    label: "SearXNG",
    descriptionEn: "Self-hosted metasearch endpoint",
    descriptionZh: "自托管聚合搜索服务",
    icon: Network
  },
  {
    id: "duckduckgo",
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
