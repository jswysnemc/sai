import { describe, expect, it } from "vitest";
import { splitGitPatchHunks } from "./partial-diff";

describe("splitGitPatchHunks", () => {
  it("keeps file headers on every independent hunk", () => {
    const patch = [
      "diff --git a/file.txt b/file.txt",
      "index 111..222 100644",
      "--- a/file.txt",
      "+++ b/file.txt",
      "@@ -1 +1 @@",
      "-one",
      "+first",
      "@@ -10 +10 @@",
      "-ten",
      "+last",
      ""
    ].join("\n");

    const hunks = splitGitPatchHunks(patch);

    expect(hunks).toHaveLength(2);
    expect(hunks[0].patch).toContain("index 111..222 100644\n--- a/file.txt\n+++ b/file.txt");
    expect(hunks[0].patch).toContain("@@ -1 +1 @@");
    expect(hunks[0].patch).not.toContain("@@ -10 +10 @@");
    expect(hunks[1].patch).toContain("@@ -10 +10 @@");
    expect(hunks[1].path).toBe("file.txt");
  });

  it("separates hunks from different files", () => {
    const patch = [
      "diff --git a/a.txt b/a.txt",
      "--- a/a.txt",
      "+++ b/a.txt",
      "@@ -1 +1 @@",
      "-a",
      "+A",
      "diff --git a/b.txt b/b.txt",
      "--- a/b.txt",
      "+++ b/b.txt",
      "@@ -1 +1 @@",
      "-b",
      "+B"
    ].join("\n");

    expect(splitGitPatchHunks(patch).map((hunk) => hunk.path)).toEqual(["a.txt", "b.txt"]);
  });
});
