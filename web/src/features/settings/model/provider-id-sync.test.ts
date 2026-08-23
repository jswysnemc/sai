import { describe, expect, it } from "vitest";
import { providerIdFollowsName, suggestedProviderId } from "./provider-id-sync";

describe("providerIdFollowsName", () => {
  it("follows generated placeholder ids", () => {
    expect(providerIdFollowsName("provider", "新供应商")).toBe(true);
    expect(providerIdFollowsName("provider-3", "OpenRouter")).toBe(true);
  });

  it("follows when id already matches the name", () => {
    expect(providerIdFollowsName("OpenRouter", "OpenRouter")).toBe(true);
  });

  it("stops following a custom id", () => {
    expect(providerIdFollowsName("or-prod", "OpenRouter")).toBe(false);
  });
});

describe("suggestedProviderId", () => {
  it("uses the trimmed display name", () => {
    expect(suggestedProviderId(" OpenRouter ", [{ id: "provider" }], 0)).toEqual({
      id: "OpenRouter",
      conflict: false
    });
  });

  it("flags a name that collides with another provider id", () => {
    expect(suggestedProviderId("openai", [{ id: "provider" }, { id: "openai" }], 0)).toEqual({
      id: "openai",
      conflict: true
    });
  });

  it("allows keeping the current provider id", () => {
    expect(suggestedProviderId("openai", [{ id: "openai" }], 0)).toEqual({
      id: "openai",
      conflict: false
    });
  });
});
