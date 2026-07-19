import { describe, expect, it } from "vitest";
import { applyCommitConfig, resolveMainCommitKind } from "./commit-behavior";

describe("resolveMainCommitKind", () => {
  it("prefers staged changes and supports enabled Smart Commit", () => {
    expect(resolveMainCommitKind(1, 2, false, true)).toBe("staged");
    expect(resolveMainCommitKind(0, 2, true, false)).toBe("all");
  });

  it("prompts or disables when Smart Commit is not enabled", () => {
    expect(resolveMainCommitKind(0, 2, false, true)).toBe("suggest_all");
    expect(resolveMainCommitKind(0, 2, false, false)).toBe("disabled");
    expect(resolveMainCommitKind(0, 0, true, true)).toBe("disabled");
  });
});

describe("applyCommitConfig", () => {
  it("applies the default post-commit command without overriding explicit variants", () => {
    expect(applyCommitConfig({}, { post_commit_command: "push", untracked_changes: "separate" })).toEqual({
      post_action: "push",
      exclude_untracked: undefined
    });
    expect(applyCommitConfig({ post_action: "sync" }, { post_commit_command: "push", untracked_changes: "separate" }).post_action).toBe("sync");
  });

  it("excludes hidden untracked files from Commit All", () => {
    expect(applyCommitConfig({ all: true }, { post_commit_command: "none", untracked_changes: "hidden" }).exclude_untracked).toBe(true);
    expect(applyCommitConfig({ all: true }, { post_commit_command: "none", untracked_changes: "mixed" }).exclude_untracked).toBeUndefined();
  });
});
