import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchGitDiffStats } from "./git-stats-client";

describe("Git overview statistics", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("使用轻量接口并完整编码所选仓库路径", async () => {
    const request = vi.fn().mockResolvedValue(new Response(JSON.stringify({ added: 12, removed: 3 })));
    vi.stubGlobal("fetch", request);
    const root = "C:/项目/one & two";

    expect(await fetchGitDiffStats(root)).toEqual({ added: 12, removed: 3 });
    const url = new URL(request.mock.calls[0][0], "http://localhost");
    expect(url.pathname).toBe("/api/workspace/git/diff-stats");
    expect(url.searchParams.get("repo_root")).toBe(root);
  });
});
