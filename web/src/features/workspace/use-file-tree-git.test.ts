import { describe, expect, it } from "vitest";
import type { GitStatusEntry } from "../../api/contracts";
import { fileTreeGitSection, fileTreeGitStatusLabel } from "./use-file-tree-git";

/**
 * 构造文件树 Git 映射测试使用的状态。
 *
 * @param patch 需要覆盖的状态字段
 * @returns 完整 Git 文件状态
 */
function entry(patch: Partial<GitStatusEntry>): GitStatusEntry {
  return {
    path: "src/main.rs",
    index_status: ".",
    worktree_status: "M",
    kind: "file",
    staged: false,
    conflicted: false,
    untracked: false,
    ...patch
  };
}

describe("file tree Git mapping", () => {
  it("按冲突、未跟踪、暂存和工作区变更划分操作分区", () => {
    expect(fileTreeGitSection(entry({ conflicted: true }))).toBe("merge");
    expect(fileTreeGitSection(entry({ untracked: true }))).toBe("untracked");
    expect(fileTreeGitSection(entry({ staged: true, index_status: "M", worktree_status: "." }))).toBe("staged");
    expect(fileTreeGitSection(entry({}))).toBe("changes");
  });

  it("使用紧凑且可区分的状态徽标", () => {
    expect(fileTreeGitStatusLabel(entry({ conflicted: true }))).toBe("U");
    expect(fileTreeGitStatusLabel(entry({ untracked: true }))).toBe("?");
    expect(fileTreeGitStatusLabel(entry({ staged: true, index_status: "M" }))).toBe("M*");
    expect(fileTreeGitStatusLabel(entry({ index_status: "A", worktree_status: "." }))).toBe("A");
    expect(fileTreeGitStatusLabel(entry({ worktree_status: "D" }))).toBe("D");
  });
});
