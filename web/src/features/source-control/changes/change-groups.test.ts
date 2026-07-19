import { describe, expect, it } from "vitest";
import type { GitStatusEntry } from "../../../api/contracts";
import { groupGitChanges } from "./change-groups";

/**
 * 创建测试用 Git 状态条目。
 *
 * @param overrides 待覆盖字段
 * @returns 完整状态条目
 */
function entry(overrides: Partial<GitStatusEntry>): GitStatusEntry {
  return {
    path: "file.txt",
    old_path: null,
    index_status: ".",
    worktree_status: ".",
    kind: "modified",
    staged: false,
    conflicted: false,
    untracked: false,
    ...overrides
  };
}

describe("groupGitChanges", () => {
  it("keeps partially staged files in staged and changes", () => {
    const partial = entry({ path: "partial.txt", index_status: "M", worktree_status: "M", staged: true });
    const groups = groupGitChanges([partial]);

    expect(groups.staged).toEqual([partial]);
    expect(groups.changes).toEqual([partial]);
  });

  it("isolates conflicts and untracked files", () => {
    const conflict = entry({ path: "conflict.txt", conflicted: true, index_status: "U", worktree_status: "U" });
    const untracked = entry({ path: "new.txt", untracked: true, index_status: "?", worktree_status: "?" });
    const groups = groupGitChanges([conflict, untracked]);

    expect(groups.conflicts).toEqual([conflict]);
    expect(groups.untracked).toEqual([untracked]);
    expect(groups.staged).toEqual([]);
    expect(groups.changes).toEqual([]);
  });
});
