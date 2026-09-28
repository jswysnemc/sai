import { describe, expect, it } from "vitest";
import type { ProviderConfig } from "../../../api/contracts";
import { mergeProviderModels } from "./merge-provider-models";

describe("provider model imports", () => {
  it("preserves manual overrides and only imports selected models", () => {
    const provider: ProviderConfig = { id: "provider", display_name: "Provider", base_url: "https://example.com", models: ["existing"], default_model: "existing", model_metadata: { existing: { context_chars: 1000, thinking_levels: ["high"], tags: ["custom"] } } };
    const before = JSON.stringify(provider);
    const patch = mergeProviderModels(provider, ["existing", "new", "new"], {
      existing: { context_chars: 2000, max_output_tokens: 500, thinking_levels: ["low"], tags: ["tool"] },
      new: { context_chars: 3000 }, ignored: { context_chars: 4000 }
    });
    expect(patch.models).toEqual(["existing", "new"]);
    expect(patch.default_model).toBe("existing");
    expect(patch.model_metadata?.existing).toEqual({ context_chars: 1000, max_output_tokens: 500, thinking_levels: ["high"], tags: ["custom", "tool"] });
    expect(patch.model_metadata?.ignored).toBeUndefined();
    expect(JSON.stringify(provider)).toBe(before);
  });
});
