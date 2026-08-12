import { describe, expect, it } from "vitest";
import { resolveGitReviewDiffMode } from "./diff-mode";

describe("resolveGitReviewDiffMode", () => {
  it("reviews all uncommitted changes through the working tree", () => {
    expect(resolveGitReviewDiffMode("changes")).toBe("working_tree");
  });

  it("keeps branch comparison for the baseline view", () => {
    expect(resolveGitReviewDiffMode("branch")).toBe("branch");
  });
});
