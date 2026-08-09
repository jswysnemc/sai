import { describe, expect, it } from "vitest";
import { encodeGitHubPath, gitHubCommitUrl, gitHubFileUrl, normalizeGitHubRepositoryUrl } from "./github-url";

describe("normalizeGitHubRepositoryUrl", () => {
  it("normalizes ssh remotes with and without the .git suffix", () => {
    expect(normalizeGitHubRepositoryUrl("git@github.com:owner/repo.git")).toBe("https://github.com/owner/repo");
    expect(normalizeGitHubRepositoryUrl("git@github.com:owner/repo")).toBe("https://github.com/owner/repo");
  });

  it("normalizes https remotes and drops extra path segments", () => {
    expect(normalizeGitHubRepositoryUrl("https://github.com/owner/repo.git")).toBe("https://github.com/owner/repo");
    expect(normalizeGitHubRepositoryUrl("https://www.github.com/owner/repo/tree/main")).toBe(
      "https://github.com/owner/repo"
    );
  });

  it("returns an empty string for non-github or incomplete remotes", () => {
    expect(normalizeGitHubRepositoryUrl("git@gitlab.com:owner/repo.git")).toBe("");
    expect(normalizeGitHubRepositoryUrl("https://gitlab.com/owner/repo.git")).toBe("");
    expect(normalizeGitHubRepositoryUrl("https://github.com/owner")).toBe("");
    expect(normalizeGitHubRepositoryUrl("   ")).toBe("");
    expect(normalizeGitHubRepositoryUrl("not a url")).toBe("");
  });
});

describe("gitHubCommitUrl", () => {
  it("builds the commit page link", () => {
    expect(gitHubCommitUrl("git@github.com:owner/repo.git", "abc123")).toBe(
      "https://github.com/owner/repo/commit/abc123"
    );
  });

  it("returns an empty string when the remote or sha is unusable", () => {
    expect(gitHubCommitUrl("git@gitlab.com:owner/repo.git", "abc123")).toBe("");
    expect(gitHubCommitUrl("git@github.com:owner/repo.git", "  ")).toBe("");
  });
});

describe("encodeGitHubPath", () => {
  it("encodes each segment and normalizes separators", () => {
    expect(encodeGitHubPath("src\\a b/c#d.ts")).toBe("src/a%20b/c%23d.ts");
  });

  it("drops empty segments", () => {
    expect(encodeGitHubPath("/src//main.ts")).toBe("src/main.ts");
  });
});

describe("gitHubFileUrl", () => {
  it("builds the blob link for a modified file", () => {
    expect(gitHubFileUrl("https://github.com/owner/repo", "abc123", "src/main.ts", "M")).toBe(
      "https://github.com/owner/repo/blob/abc123/src/main.ts"
    );
  });

  it("returns an empty string for deleted files", () => {
    expect(gitHubFileUrl("https://github.com/owner/repo", "abc123", "src/main.ts", "D")).toBe("");
  });

  it("returns an empty string when the remote is not github", () => {
    expect(gitHubFileUrl("https://gitlab.com/owner/repo", "abc123", "src/main.ts", "M")).toBe("");
  });
});
