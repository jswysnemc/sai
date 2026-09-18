import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts/config";
import { DialogProvider } from "../../../shared/ui/dialog/dialog-provider";
import { ModelEndpointSettings } from "./model-endpoint-settings";

describe("model connection settings", () => {
  it("shows only the selected type and masks saved credentials", () => {
    const config = { gateways: { qq: { enabled: false, transport: "", listen: "", base_url: "", token: "", app_id: "", client_secret: "" }, weixin: { enabled: false, base_url: "", cdn_base_url: "", bot_type: "", token: "", account: "", bot_agent: "" } }, providers: [], active_provider: "llm", model_endpoints: [
      { id: "image", kind: "image_generation", name: "Image connection", endpoint: "https://images.example/generate", api_key: "image-secret", model: "image-model" },
      { id: "jev", kind: "jev", name: "JEV connection", endpoint: "https://decisions.example:8443/decide", api_key: "SAVED_SECRET", model: "jev-latest" }
    ] } as AppConfig;
    const html = renderToStaticMarkup(<DialogProvider><ModelEndpointSettings kind="jev" config={config} secretSentinel="SAVED_SECRET" onChange={() => {}} /></DialogProvider>);
    expect(html).toContain("JEV connection");
    expect(html).toContain("https://decisions.example:8443/decide");
    expect(html).toContain("jev-latest");
    expect(html).toContain('type="password"');
    expect(html).not.toContain("SAVED_SECRET");
    expect(html).not.toContain("image-secret");
    expect(html).not.toContain("Image connection");
  });
});
