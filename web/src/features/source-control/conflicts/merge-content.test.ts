import { describe, expect, it } from "vitest";
import { combineConflictBlocks } from "./merge-content";

describe("combineConflictBlocks", () => {
  it("merges conflict blocks without duplicating common content", () => {
    const current = "header\n<<<<<<< HEAD\nours\n=======\ntheirs\n>>>>>>> feature\nfooter\n";
    expect(combineConflictBlocks(current)).toBe("header\nours\ntheirs\nfooter\n");
  });

  it("ignores the base section in diff3 conflict markers", () => {
    const current = "<<<<<<< HEAD\nours\n||||||| base\nbase\n=======\ntheirs\n>>>>>>> feature\n";
    expect(combineConflictBlocks(current)).toBe("ours\ntheirs\n");
  });

  it("rejects text without a complete conflict block", () => {
    expect(combineConflictBlocks("resolved\n")).toBeNull();
    expect(combineConflictBlocks("<<<<<<< HEAD\nours\n")).toBeNull();
  });
});
