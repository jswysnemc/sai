import { describe, expect, it } from "vitest";
import type { GitStatusEntry } from "../../../api/contracts";
import { buildGitChangeTreeRows } from "./change-tree";

/**
 * 创建树形视图测试条目。
 *
 * @param path 仓库相对路径
 * @returns 完整 Git 状态条目
 */
function entry(path: string): GitStatusEntry {
  return {
    path,
    old_path: null,
    index_status: ".",
    worktree_status: "M",
    kind: "modified",
    staged: false,
    conflicted: false,
    untracked: false
  };
}

describe("buildGitChangeTreeRows", () => {
  it("orders directories before files and preserves depth", () => {
    const rows = buildGitChangeTreeRows([
      entry("README.md"),
      entry("src/z.ts"),
      entry("src/components/a.tsx")
    ]);

    expect(rows.map((row) => `${row.kind}:${row.depth}:${row.kind === "file" ? row.name : row.path}`)).toEqual([
      "directory:0:src",
      "directory:1:src/components",
      "file:2:a.tsx",
      "file:1:z.ts",
      "file:0:README.md"
    ]);
  });

  it("supports Windows separators and collapsed directories", () => {
    const rows = buildGitChangeTreeRows(
      [entry("src\\nested\\file.ts"), entry("top.ts")],
      new Set(["src"])
    );

    expect(rows.map((row) => row.kind === "directory" ? row.path : row.name)).toEqual([
      "src",
      "top.ts"
    ]);
  });
});
