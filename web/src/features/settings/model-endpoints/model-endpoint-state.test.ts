import { describe, expect, it } from "vitest";
import type { AppConfig, ModelEndpointConfig } from "../../../api/contracts/config";
import { newModelEndpoint, updateModelEndpoint } from "./model-endpoint-state";

describe("independent model connections", () => {
  it("edits one endpoint without changing other endpoints or LLM selection", () => {
    const image = newModelEndpoint([], "image_generation", "Image");
    const jev = newModelEndpoint([image], "jev", "JEV");
    const config = { providers: [{ id: "llm", base_url: "https://chat.example/v1" }], active_provider: "llm", model_endpoints: [image, jev] } as AppConfig;
    const next = updateModelEndpoint(config, jev.id, { endpoint: "http://localhost:9100/decide", api_key: "jev-key" });
    expect(next.providers).toBe(config.providers);
    expect(next.active_provider).toBe("llm");
    expect(next.model_endpoints?.[0]).toBe(image);
    expect(next.model_endpoints?.[1]).toMatchObject({ kind: "jev", endpoint: "http://localhost:9100/decide", api_key: "jev-key" });
    expect(jev.api_key).toBe("");
  });

  it("keeps stable unique IDs after deletion and supports separate types", () => {
    const retained = { ...newModelEndpoint([], "jev", "JEV"), id: "jev-2" };
    const entries: ModelEndpointConfig[] = [retained];
    const first = newModelEndpoint(entries, "jev", "JEV");
    expect(first.id).toBe("jev-1");
    expect(newModelEndpoint([...entries, first], "jev", "JEV").id).toBe("jev-3");
    expect(newModelEndpoint(entries, "image_generation", "Image").kind).toBe("image_generation");
  });
});
