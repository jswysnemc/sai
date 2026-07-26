import { describe, expect, it } from "vitest";
import { classifyPluginField, groupPluginFields, isPluginEnabledField } from "./plugin-field-groups";

describe("classifyPluginField", () => {
  it("routes credential-like names to credentials", () => {
    expect(classifyPluginField("tavily_api_keys", [])).toBe("credentials");
    expect(classifyPluginField("access_token", "")).toBe("credentials");
  });

  it("routes address and path names to endpoints", () => {
    expect(classifyPluginField("searxng_base_url", "")).toBe("endpoints");
    expect(classifyPluginField("output_dir", "")).toBe("endpoints");
  });

  it("routes numeric guards to limits regardless of type", () => {
    expect(classifyPluginField("timeout_seconds", 30)).toBe("limits");
    expect(classifyPluginField("max_results", 5)).toBe("limits");
    expect(classifyPluginField("thinking_depth", "deep")).toBe("limits");
  });

  it("routes plain booleans to switches", () => {
    expect(classifyPluginField("safe_search", true)).toBe("switches");
  });

  it("keeps unmatched fields in other", () => {
    expect(classifyPluginField("style", "compact")).toBe("other");
  });
});

describe("groupPluginFields", () => {
  it("excludes the enable toggle and orders groups predictably", () => {
    const groups = groupPluginFields({
      enabled: true,
      safe_search: true,
      tavily_api_keys: [],
      max_results: 5,
      searxng_base_url: ""
    });
    expect(groups.map((group) => group.id)).toEqual([
      "credentials",
      "endpoints",
      "limits",
      "switches"
    ]);
    expect(groups.flatMap((group) => group.entries.map(([name]) => name))).not.toContain("enabled");
  });

  it("omits empty groups", () => {
    const groups = groupPluginFields({ enabled: true, safe_search: false });
    expect(groups).toHaveLength(1);
    expect(groups[0].id).toBe("switches");
  });

  it("returns nothing when only the toggle exists", () => {
    expect(groupPluginFields({ enabled: true })).toEqual([]);
  });
});

describe("isPluginEnabledField", () => {
  it("matches only the exact toggle name", () => {
    expect(isPluginEnabledField("enabled")).toBe(true);
    expect(isPluginEnabledField("vision_screening_enabled")).toBe(false);
  });
});
