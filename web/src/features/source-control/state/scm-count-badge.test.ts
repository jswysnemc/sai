import { describe, expect, it } from "vitest";
import type { GitStatusEntry } from "../../../api/contracts";
import { resolveScmCountBadge } from "./scm-count-badge";

/**
 * 创建角标测试文件条目。
 *
 * @param path 仓库相对路径
 * @param untracked 是否未跟踪
 * @returns 完整 Git 状态条目
 */
function entry(path: string, untracked = false): GitStatusEntry {
  return {
    path,
    old_path: null,
    index_status: untracked ? "?" : ".",
    worktree_status: untracked ? "?" : "M",
    kind: untracked ? "untracked" : "modified",
    staged: false,
    conflicted: false,
    untracked
  };
}

describe("resolveScmCountBadge", () => {
  const repositories = [
    { repo_root: "/first", entries: [entry("a.ts"), entry("new.ts", true)] },
    { repo_root: "/second", entries: [entry("b.ts")] }
  ];

  it("counts all or focused repositories", () => {
    expect(resolveScmCountBadge("all", repositories, "/first", "separate")).toBe(3);
    expect(resolveScmCountBadge("focused", repositories, "/first", "separate")).toBe(2);
  });

  it("respects hidden untracked files and disabled badges", () => {
    expect(resolveScmCountBadge("all", repositories, "/first", "hidden")).toBe(2);
    expect(resolveScmCountBadge("off", repositories, "/first", "separate")).toBeNull();
  });
});
