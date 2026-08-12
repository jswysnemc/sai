import { describe, expect, it } from "vitest";
import type { GitStatusEntry } from "../../api/contracts";
import { directoryGitTones, fileTreeGitSection, fileTreeGitStatusLabel } from "./use-file-tree-git";

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

  it("使用与 VS Code 对齐的状态徽标", () => {
    expect(fileTreeGitStatusLabel(entry({ conflicted: true }))).toBe("!");
    expect(fileTreeGitStatusLabel(entry({ untracked: true }))).toBe("U");
    expect(fileTreeGitStatusLabel(entry({ staged: true, index_status: "M" }))).toBe("M*");
    expect(fileTreeGitStatusLabel(entry({ index_status: "A", worktree_status: "." }))).toBe("A");
    expect(fileTreeGitStatusLabel(entry({ worktree_status: "D" }))).toBe("D");
  });
});

describe("directoryGitTones", () => {
  /**
   * 构造指定状态形态的最小 Git 条目。
   *
   * @param overrides 状态字段覆盖
   * @returns 文件树 Git 条目
   */
  const entryOf = (overrides: Partial<{ conflicted: boolean; untracked: boolean; index_status: string; worktree_status: string }>) => ({
    repoRoot: "/repo",
    entry: {
      path: "x",
      index_status: ".",
      worktree_status: "M",
      staged: false,
      untracked: false,
      conflicted: false,
      ...overrides
    }
  }) as unknown as import("./use-file-tree-git").FileTreeGitEntry;

  it("把文件状态冒泡到全部祖先目录", () => {
    const tones = directoryGitTones(new Map([
      ["web/src/app/main.tsx", entryOf({})]
    ]));

    expect(tones.get("web")).toBe("modified");
    expect(tones.get("web/src")).toBe("modified");
    expect(tones.get("web/src/app")).toBe("modified");
    expect(tones.has("web/src/app/main.tsx")).toBe(false);
  });

  it("同目录多状态按优先级保留最高色调", () => {
    const tones = directoryGitTones(new Map([
      ["src/a.rs", entryOf({})],
      ["src/b.rs", entryOf({ untracked: true })]
    ]));

    // 新增优先于修改
    expect(tones.get("src")).toBe("added");
  });
});
