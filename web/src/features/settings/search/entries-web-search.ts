import { getSearchProvider } from "../web-search/search-provider-catalog";
import { SEARCH_PROVIDER_FIELDS } from "../web-search/search-provider-fields-data";
import { WEB_SEARCH_PROVIDER_IDS } from "../web-search/web-search-config";
import type { SettingsSearchEntry } from "./settings-search-types";

/** 网页搜索字段索引，供应商字段携带对象标识以保证定位正确。 */
export const WEB_SEARCH_ENTRIES: SettingsSearchEntry[] = [
  ...[["enabled", "Enable Web search", "启用 Web 搜索"], ["default_provider", "Default search provider", "默认搜索供应商"], ["max_results", "Maximum results", "最大结果数量"], ["timeout_seconds", "Search timeout", "搜索超时"]].map(([key, labelEn, labelZh]): SettingsSearchEntry => ({ anchor: `web-search.${key}`, section: "web-search", labelEn, labelZh, keywords: [`plugins.web.${key}`] })),
  ...WEB_SEARCH_PROVIDER_IDS.flatMap((id): SettingsSearchEntry[] => {
    if (id === "duckduckgo") return [];
    const provider = getSearchProvider(id);
    const fields = [
      { key: `${id}_base_url`, en: "API endpoint", zh: "接口地址" },
      ...(id !== "searxng" ? [{ key: `${id}_api_keys`, en: "API keys", zh: "接口密钥" }] : []),
      ...SEARCH_PROVIDER_FIELDS[id]
    ];
    return fields.map((field) => ({ anchor: `web-search.${field.key}`, section: "web-search", item: id, labelEn: `${provider.label} · ${field.en}`, labelZh: `${provider.label} · ${field.zh}`, keywords: [`plugins.web.${field.key}`] }));
  })
];
