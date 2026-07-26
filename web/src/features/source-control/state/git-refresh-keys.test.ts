import { describe, expect, it } from "vitest";
import { allRefreshKeys, refreshKeysFor } from "./git-refresh-keys";

describe("refreshKeysFor", () => {
  it("暂存文件不触发提交历史与远端资源重取", () => {
    const keys = refreshKeysFor("stage");
    expect(keys).toContain("git-status");
    expect(keys).toContain("workspace-diff");
    expect(keys).not.toContain("git-log");
    expect(keys).not.toContain("git-resources");
    expect(keys).not.toContain("git-repositories");
  });

  it("提交后刷新历史与分支追踪状态", () => {
    const keys = refreshKeysFor("commit");
    expect(keys).toContain("git-log");
    expect(keys).toContain("git-branches");
    expect(keys).toContain("git-status");
  });

  it("拉取覆盖远端相关的全部数据", () => {
    const keys = refreshKeysFor("pull");
    expect(keys).toContain("git-resources");
    expect(keys).toContain("git-log");
    expect(keys).toContain("file-tree");
  });

  it("丢弃暂存不重取提交历史", () => {
    expect(refreshKeysFor("stash_drop")).toEqual(["git-resources"]);
  });

  it("未归类的操作按全量失效处理", () => {
    expect(refreshKeysFor("switch_branch")).toEqual(allRefreshKeys());
  });
});
