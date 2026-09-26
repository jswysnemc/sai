import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import {
  hiddenSearchApiKeyCount,
  mergeSearchApiKeyText,
  normalizeWebSearchSelection,
  readWebSearchConfig,
  visibleSearchApiKeys,
  webSearchProviderAvailable,
  webSearchProviderStatus,
  writeWebSearchConfig
} from "./web-search-config";

const baseConfig = {
  active_provider: "test",
  providers: [],
  gateways: {},
  plugins: {
    web: { enabled: true, tavily_api_keys: ["saved-key"], searxng_base_url: "https://search.example.test" },
    vision: { enabled: true }
  }
} as unknown as AppConfig;

describe("web search config", () => {
  it("补齐旧版配置缺少的供应商详细字段", () => {
    const config = readWebSearchConfig(baseConfig);

    expect(config.default_provider).toBe("auto");
    expect(config.max_results).toBe(5);
    expect(config.tavily_base_url).toBe("https://api.tavily.com/search");
    expect(config.tavily_api_keys).toEqual(["saved-key"]);
    expect(config.searxng_base_url).toBe("https://search.example.test");
  });

  it("写回时保留其他 CLI 工具与未知搜索字段", () => {
    const current = {
      ...baseConfig,
      plugins: { ...baseConfig.plugins, web: { ...baseConfig.plugins?.web, custom_option: "keep" } }
    } as AppConfig;
    const next = { ...readWebSearchConfig(current), max_results: 8 };
    const updated = writeWebSearchConfig(current, next);

    expect(updated.plugins?.vision).toEqual({ enabled: true });
    expect(updated.plugins?.web.custom_option).toBe("keep");
    expect(updated.plugins?.web.max_results).toBe(8);
  });

  it("关闭当前默认供应商后回退自动选择", () => {
    const current = { ...readWebSearchConfig(baseConfig), default_provider: "tavily" as const, tavily_enabled: false };

    expect(normalizeWebSearchSelection(current).default_provider).toBe("auto");
  });

  it("SearXNG 缺少实例地址时不能作为默认供应商", () => {
    const current = {
      ...readWebSearchConfig(baseConfig),
      default_provider: "searxng" as const,
      searxng_base_url: ""
    };

    expect(webSearchProviderAvailable(current, "searxng")).toBe(false);
    expect(normalizeWebSearchSelection(current).default_provider).toBe("auto");
  });

  it("区分内置、已配置和缺少配置的供应商", () => {
    const config = readWebSearchConfig(baseConfig);

    expect(webSearchProviderStatus(config, "duckduckgo")).toBe("builtin");
    expect(webSearchProviderStatus(config, "tavily")).toBe("configured");
    expect(webSearchProviderStatus(config, "tinyfish")).toBe("missing");
  });

  it("编辑密钥时隐藏服务端占位符", () => {
    const sentinel = "__SECRET__";
    const keys = [sentinel, "$env:TAVILY_API_KEY"];

    expect(visibleSearchApiKeys(keys, sentinel)).toEqual(["$env:TAVILY_API_KEY"]);
    expect(hiddenSearchApiKeyCount(keys, sentinel)).toBe(1);
    expect(mergeSearchApiKeyText(keys, "$env:TAVILY_API_KEY\nnew-key", sentinel)).toEqual([
      sentinel,
      "$env:TAVILY_API_KEY",
      "new-key"
    ]);
  });

  it("保留隐藏密钥的数组位置以便服务端正确恢复", () => {
    const sentinel = "__SECRET__";
    const keys = ["$env:TAVILY_API_KEY", sentinel];

    expect(mergeSearchApiKeyText(keys, "$env:TAVILY_NEXT_KEY", sentinel)).toEqual([
      "$env:TAVILY_NEXT_KEY",
      sentinel
    ]);
    expect(mergeSearchApiKeyText(keys, "", sentinel)).toEqual(["", sentinel]);
  });
});
