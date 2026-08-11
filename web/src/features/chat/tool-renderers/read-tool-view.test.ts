import { describe, expect, it } from "vitest";
import { pathsReferToSameFile } from "./read-tool-view";

describe("pathsReferToSameFile", () => {
  it("matches identical paths", () => {
    expect(pathsReferToSameFile("/w/a.txt", "/w/a.txt")).toBe(true);
  });

  it("matches absolute path with basename shown in the card header", () => {
    expect(pathsReferToSameFile("/home/snemc/workspace/tmp/sandbox/焦尾歌.txt", "焦尾歌.txt")).toBe(true);
  });

  it("matches relative workspace path with absolute result path", () => {
    expect(
      pathsReferToSameFile(
        "/home/snemc/workspace/tmp/sandbox/docs/a.md",
        "docs/a.md"
      )
    ).toBe(true);
  });

  it("rejects unrelated files", () => {
    expect(pathsReferToSameFile("/w/a.txt", "/w/b.txt")).toBe(false);
  });
});
