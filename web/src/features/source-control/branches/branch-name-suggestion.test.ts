import { describe, expect, it } from "vitest";
import { createBranchNameSuggestion } from "./branch-name-suggestion";

describe("createBranchNameSuggestion", () => {
  it("creates a Git-safe stable suggestion from an injected source", () => {
    expect(createBranchNameSuggestion(() => 0)).toBe("bright-anchor-000");
    expect(createBranchNameSuggestion(() => 0.999999)).toBe("vivid-summit-zzz");
  });

  it("clamps invalid random values", () => {
    expect(createBranchNameSuggestion(() => Number.NaN)).toBe("bright-anchor-000");
    expect(createBranchNameSuggestion(() => -1)).toBe("bright-anchor-000");
  });
});
