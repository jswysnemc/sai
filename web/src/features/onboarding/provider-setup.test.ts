import { describe, expect, it } from "vitest";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { AppConfig, ProviderConfig } from "../../api/contracts";
import { createProviderSetupDraft, needsProviderSetup, providerSetupInput, providerSetupIssue } from "./provider-setup-draft";
import { ProviderSetupGate } from "./provider-setup-gate";

const provider: ProviderConfig = {
  id: "test", display_name: "Test provider", base_url: "https://example.com/v1",
  protocol: "openai-chat", default_model: "test-model", api_key: "__UNCHANGED__"
};

describe("首次供应商配置", () => {
  it("完成后的后台配置刷新失败不卸载工作台或丢弃编辑状态", () => {
    const client = new QueryClient({ defaultOptions: { queries: { retry: false, staleTime: Infinity } } });
    client.setQueryData(["config"], { config: { provider_setup_complete: true, providers: [provider] }, secret_sentinel: "hidden" });
    client.getQueryCache().find({ queryKey: ["config"] })!.setState({ status: "error", error: new Error("offline") });
    const html = renderToStaticMarkup(createElement(QueryClientProvider, { client },
      createElement(ProviderSetupGate, { children: createElement("p", null, "existing workbench") })
    ));
    expect(html).toContain("existing workbench");
    expect(html).not.toContain("offline");
    client.clear();
  });

  it("仅引导明确未完成的新配置，保留旧版配置兼容性", () => {
    const config = { active_provider: provider.id, providers: [provider] } as AppConfig;
    expect(needsProviderSetup(config)).toBe(false);
    expect(needsProviderSetup({ ...config, provider_setup_complete: false })).toBe(true);
    expect(needsProviderSetup({ ...config, provider_setup_complete: true })).toBe(false);
  });

  it("不把脱敏标记作为密钥提交，环境变量引用保持可编辑", () => {
    const draft = createProviderSetupDraft(provider, "__UNCHANGED__");
    expect(draft.api_key).toBe("");
    expect(providerSetupInput(draft).api_key).toBeUndefined();
    expect(createProviderSetupDraft({ ...provider, api_key: "$env:SETUP_KEY" }, "__UNCHANGED__").api_key).toBe("$env:SETUP_KEY");
    expect(providerSetupInput({ ...draft, api_key: " fresh-key " }).api_key).toBe("fresh-key");
  });

  it("切到自定义供应商时清空旧模板和密钥，避免意外发往旧地址", () => {
    const draft = createProviderSetupDraft(undefined, "__UNCHANGED__");
    expect(draft).toEqual({ provider_id: null, display_name: "", base_url: "", protocol: "auto", api_key: "", model: "" });
  });

  it("缺少基本字段时给出具体原因，并拦截非 HTTP 地址", () => {
    const draft = createProviderSetupDraft(provider, "__UNCHANGED__");
    expect(providerSetupIssue(draft)).toBeNull();
    expect(providerSetupIssue({ ...draft, display_name: " " })).toBe("name");
    expect(providerSetupIssue({ ...draft, base_url: "file:///tmp/api" })).toBe("url");
    expect(providerSetupIssue({ ...draft, base_url: "bad-address" })).toBe("url");
    expect(providerSetupIssue({ ...draft, model: " " })).toBe("model");
    expect(providerSetupIssue({ ...draft, base_url: "http://localhost:11434/v1" })).toBeNull();
  });
});
