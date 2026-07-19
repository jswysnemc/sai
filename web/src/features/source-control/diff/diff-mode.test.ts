import { describe, expect, it } from "vitest";
import { resolveGitReviewDiffMode } from "./diff-mode";

describe("resolveGitReviewDiffMode", () => {
  it("uses HEAD to index for staged files", () => {
    expect(resolveGitReviewDiffMode("changes", "staged")).toBe("staged");
  });

  it("uses index to worktree for working and conflict files", () => {
    expect(resolveGitReviewDiffMode("changes", "changes")).toBe("unstaged");
    expect(resolveGitReviewDiffMode("changes", "untracked")).toBe("unstaged");
    expect(resolveGitReviewDiffMode("changes", "merge")).toBe("unstaged");
  });

  it("keeps branch comparison independent from file section", () => {
    expect(resolveGitReviewDiffMode("branch", "staged")).toBe("branch");
  });
});
