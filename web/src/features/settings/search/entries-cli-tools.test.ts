import { describe, expect, it } from "vitest";
import type { AppConfig } from "../../../api/contracts";
import { buildCliToolSearchEntries } from "./entries-cli-tools";
import { searchEntryHref } from "./settings-search";

describe("CLI field search", () => {
  it("indexes nested configuration names without exposing their values", () => {
    const entries = buildCliToolSearchEntries({
      plugins: {
        vision: { enabled: true, vision_provider_id: "private-credential", options: { timeout_seconds: 20 } },
        web: { api_key: "separate-search-section" }
      }
    } as unknown as AppConfig);
    expect(entries.map((entry) => entry.anchor)).toEqual([
      "cli-tools.vision.enabled",
      "cli-tools.vision.vision_provider_id",
      "cli-tools.vision.options.timeout_seconds"
    ]);
    expect(JSON.stringify(entries)).not.toContain("private-credential");
    expect(JSON.stringify(entries)).not.toContain("separate-search-section");
    expect(searchEntryHref(entries[2])).toBe("/settings/cli-tools?focus=cli-tools.vision.options.timeout_seconds&item=vision");
  });
});
