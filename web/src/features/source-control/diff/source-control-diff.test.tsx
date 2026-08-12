import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { GitDiffResponse, GitRepositoryState } from "../../../api/contracts";
import { SourceControlDiff } from "./source-control-diff";

const PATCH = [
  "diff --git a/src/app.ts b/src/app.ts",
  "index 1111111..2222222 100644",
  "--- a/src/app.ts",
  "+++ b/src/app.ts",
  "@@ -1,3 +1,4 @@",
  " const a = 1;",
  "-const b = 2;",
  "+const b = 3;",
  "+const c = 4;",
  "",
].join("\n");

/**
 * 构造最小可用的仓库状态。
 *
 * @returns 带一个修改文件和一个未跟踪二进制文件的仓库状态
 */
function repositoryState(): GitRepositoryState {
  return {
    repo_root: "/repo",
    workdir: "/repo",
    head: "main",
    has_commits: true,
    upstream: "",
    remote_name: "",
    remote_url: "",
    ahead: 0,
    behind: 0,
    stash_count: 0,
    dirty_counts: { staged: 0, unstaged: 1, untracked: 1, conflicted: 0 },
    entries: [
      {
        path: "src/app.ts",
        old_path: null,
        index_status: ".",
        worktree_status: "M",
        kind: "modified",
        staged: false,
        conflicted: false,
        untracked: false,
      },
      {
        path: "assets/logo.png",
        old_path: null,
        index_status: ".",
        worktree_status: ".",
        kind: "untracked",
        staged: false,
        conflicted: false,
        untracked: true,
      },
    ],
    operation: null,
    status: "ready",
  };
}

/**
 * 构造指定模式的 Diff 响应。
 *
 * @param mode 比较模式
 * @returns Diff 响应
 */
function diffResponse(mode: string): GitDiffResponse {
  return {
    base_ref: mode === "branch" ? "origin/main" : "HEAD",
    head_ref: "WORKTREE",
    mode,
    files: ["src/app.ts"],
    patch: PATCH,
    stat: "",
    truncated: false,
    binary_files: [],
  };
}

describe("SourceControlDiff", () => {
  it("renders a summary bar and one card per file, padding entries missing from the patch", () => {
    const html = renderToStaticMarkup(
      <SourceControlDiff
        data={diffResponse("working_tree")}
        loading={false}
        error={null}
        state={repositoryState()}
        selectedPath={null}
        busy={false}
        runOperation={async () => undefined}
      />
    );

    expect(html).toContain("git-review-summary");
    expect(html).toContain("+2");
    expect(html).toContain("-1");
    expect(html).toContain("main");
    expect(html.match(/git-file-card-head/g)?.length).toBe(2);
    expect(html).toContain("logo.png");
    expect(html).toContain("tone-modified");
    expect(html).toContain("tone-added");
    // 工作树模式提供暂存等文件级操作
    expect(html).toContain("git-file-card-action");
  });

  it("keeps the branch review read-only", () => {
    const html = renderToStaticMarkup(
      <SourceControlDiff
        data={diffResponse("branch")}
        loading={false}
        error={null}
        state={repositoryState()}
        selectedPath={null}
        busy={false}
        runOperation={async () => undefined}
      />
    );

    expect(html).toContain("origin/main");
    expect(html.match(/git-file-card-head/g)?.length).toBe(1);
    expect(html).not.toContain("git-file-card-action");
  });

  it("shows the clean state when there is nothing to review", () => {
    const html = renderToStaticMarkup(
      <SourceControlDiff
        data={{ ...diffResponse("working_tree"), patch: "", files: [] }}
        loading={false}
        error={null}
        state={{ ...repositoryState(), entries: [] }}
        selectedPath={null}
        busy={false}
        runOperation={async () => undefined}
      />
    );

    expect(html).toContain("git-diff-empty");
    expect(html).not.toContain("git-file-card");
  });
});
