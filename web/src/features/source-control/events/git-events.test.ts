import { describe, expect, it } from "vitest";
import { gitEventsUrl, parseGitWatchEvent } from "./git-events";

describe("gitEventsUrl", () => {
  it("encodes a selected repository root", () => {
    expect(gitEventsUrl("/workspace/repo with space"))
      .toBe("/api/workspace/git/events?repo_root=%2Fworkspace%2Frepo%20with%20space");
  });
});

describe("parseGitWatchEvent", () => {
  it("accepts a complete watcher event", () => {
    expect(parseGitWatchEvent(JSON.stringify({
      sequence: 3,
      workspace_root: "/workspace",
      paths: ["/workspace/file.txt"],
      paths_truncated: false,
      repository_metadata_changed: false,
      error: null
    })).sequence).toBe(3);
  });

  it("rejects incomplete watcher events", () => {
    expect(() => parseGitWatchEvent("{}"))
      .toThrow("invalid Git repository event payload");
  });
});
