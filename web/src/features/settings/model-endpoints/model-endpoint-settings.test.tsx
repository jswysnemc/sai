import { MemoryRouter } from "react-router-dom";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts/config";
import { DialogProvider } from "../../../shared/ui/dialog/dialog-provider";
import { ModelEndpointSettings } from "./model-endpoint-settings";

describe("model connection settings", () => {
  it("opens the connection identified by the shared URL", () => {
    const config = { providers: [], gateways: {}, active_provider: "", model_endpoints: [
      { id: "one", kind: "image_generation", name: "One", endpoint: "https://one.example", api_key: "", model: "one" },
      { id: "two", kind: "image_generation", name: "Two", endpoint: "https://two.example", api_key: "", model: "two" }
    ] } as unknown as AppConfig;
    const html = renderToStaticMarkup(<MemoryRouter initialEntries={["/settings/image-models?item=two"]}><DialogProvider><ModelEndpointSettings kind="image_generation" config={config} secretSentinel="hidden" onChange={() => undefined} /></DialogProvider></MemoryRouter>);
    expect(html).toContain('value="https://two.example"');
    expect(html).not.toContain('value="https://one.example"');
  });
  it("shows only the selected type and masks saved credentials", () => {
    const config = { gateways: { qq: { enabled: false, transport: "", listen: "", base_url: "", token: "", app_id: "", client_secret: "" }, weixin: { enabled: false, base_url: "", cdn_base_url: "", bot_type: "", token: "", account: "", bot_agent: "" } }, providers: [], active_provider: "llm", model_endpoints: [
      { id: "image", kind: "image_generation", name: "Image connection", endpoint: "https://images.example/generate", api_key: "image-secret", model: "image-model" },
      { id: "jev", kind: "jev", name: "JEV connection", endpoint: "https://decisions.example:8443/decide", api_key: "SAVED_SECRET", model: "jev-latest" }
    ] } as AppConfig;
    const html = renderToStaticMarkup(<MemoryRouter><DialogProvider><ModelEndpointSettings kind="jev" config={config} secretSentinel="SAVED_SECRET" onChange={() => {}} /></DialogProvider></MemoryRouter>);
    expect(html).toContain("JEV connection");
    expect(html).toContain("https://decisions.example:8443/decide");
    expect(html).toContain("jev-latest");
    expect(html).toContain("测试连接");
    expect(html).toContain("接入点");
    expect(html).toContain("凭据");
    expect(html).not.toContain('type="password"');
    expect(html).toContain("编辑密钥");
    expect(html).not.toContain("SAVED_SECRET");
    expect(html).not.toContain("image-secret");
    expect(html).not.toContain("Image connection");
  });

  it("uses the same connection groups for image models", () => {
    const config = { gateways: { qq: { enabled: false, transport: "", listen: "", base_url: "", token: "", app_id: "", client_secret: "" }, weixin: { enabled: false, base_url: "", cdn_base_url: "", bot_type: "", token: "", account: "", bot_agent: "" } }, providers: [], active_provider: "llm", model_endpoints: [
      { id: "image", kind: "image_generation", name: "Image connection", endpoint: "https://images.example/generate", api_key: "SAVED_SECRET", model: "gpt-image-1" },
      { id: "jev", kind: "jev", name: "JEV connection", endpoint: "https://decisions.example:8443/decide", api_key: "jev-secret", model: "jev-latest" }
    ] } as AppConfig;
    const html = renderToStaticMarkup(<MemoryRouter><DialogProvider><ModelEndpointSettings kind="image_generation" config={config} secretSentinel="SAVED_SECRET" onChange={() => {}} /></DialogProvider></MemoryRouter>);
    expect(html).toContain("Image connection");
    expect(html).toContain("https://images.example/generate");
    expect(html).toContain("gpt-image-1");
    expect(html).toContain("接入点");
    expect(html).toContain("凭据");
    expect(html).toContain("模型目录");
    expect(html).toContain("获取模型");
    expect(html).toContain("连通性");
    expect(html).toContain("测试生图");
    expect(html).toContain("身份");
    expect(html).toContain("显示名称");
    expect(html).not.toContain('type="password"');
    expect(html).toContain("编辑密钥");
    expect(html).not.toContain("SAVED_SECRET");
    expect(html).not.toContain("JEV connection");
    expect(html).not.toContain("jev-secret");
  });
});
