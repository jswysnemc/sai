import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { NewSessionDefaultSettings } from "./new-session-default-settings";

const config = {
  active_provider: "provider-a",
  providers: [
    {
      id: "provider-a",
      display_name: "Provider A",
      base_url: "https://example.test/v1",
      models: ["model-a"],
      default_model: "model-a",
      enabled: false
    }
  ],
  agent: { engine: "native" },
  session: {
    new_session_provider_id: "provider-a",
    new_session_model: "model-a"
  }
} as unknown as AppConfig;

describe("NewSessionDefaultSettings", () => {
  it("warns when the new-session model belongs to a disabled provider", () => {
    const html = renderToStaticMarkup(
      <NewSessionDefaultSettings config={config} onConfigChange={() => undefined} />
    );
    expect(html).toContain("所选供应商（Provider A）已停用");
    expect(html).toContain("跟随内核默认");
    expect(html).toContain("model-a");
  });
});
