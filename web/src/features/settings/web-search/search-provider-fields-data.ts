import type { WebSearchProviderId } from "./web-search-config";

/** 搜索供应商专属字段定义，同时供表单与字段搜索使用。 */
export type SearchProviderField = {
  key: string;
  kind: "text" | "switch" | "choice";
  en: string;
  zh: string;
  hintEn: string;
  hintZh: string;
  options?: Array<[string, string, string]>;
  numeric?: boolean;
};

export const SEARCH_PROVIDER_FIELDS: Record<Exclude<WebSearchProviderId, "duckduckgo">, SearchProviderField[]> = {
  tinyfish: [
    { key: "tinyfish_default_location", kind: "text", en: "Default location", zh: "默认位置", hintEn: "Leave empty to omit location.", hintZh: "留空则不指定位置。" },
    { key: "tinyfish_default_language", kind: "text", en: "Default language", zh: "默认语言", hintEn: "Leave empty to use the service default.", hintZh: "留空则使用服务默认语言。" }
  ],
  tavily: [
    { key: "tavily_search_depth", kind: "choice", en: "Search depth", zh: "搜索深度", hintEn: "Advanced search retrieves more detail with higher latency.", hintZh: "深入搜索提供更详细的结果，耗时相应增加。", options: [["basic", "Basic", "基础"], ["advanced", "Advanced", "深入"]] },
    { key: "tavily_include_answer", kind: "switch", en: "Include generated answer", zh: "附带生成答案", hintEn: "Include Tavily's synthesized answer.", hintZh: "请求 Tavily 返回综合答案。" },
    { key: "tavily_include_raw_content", kind: "switch", en: "Include raw content", zh: "附带原始正文", hintEn: "Include extracted page content.", hintZh: "在结果中附带提取后的页面正文。" }
  ],
  firecrawl: [
    { key: "firecrawl_only_main_content", kind: "switch", en: "Only main content", zh: "仅保留主要正文", hintEn: "Remove navigation, footer and other page chrome.", hintZh: "移除导航、页脚等非正文内容。" }
  ],
  anysearch: [],
  searxng: [
    { key: "searxng_language", kind: "text", en: "Language", zh: "语言", hintEn: "Use auto or a supported SearXNG language code.", hintZh: "填写 auto 或 SearXNG 支持的语言代码。" },
    { key: "searxng_safe_search", kind: "choice", en: "Safe search", zh: "安全搜索", hintEn: "Filter explicit search results.", hintZh: "设置搜索结果的内容过滤级别。", options: [["0", "Off", "关闭"], ["1", "Moderate", "适中"], ["2", "Strict", "严格"]], numeric: true }
  ]
};
