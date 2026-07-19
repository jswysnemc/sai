import { describe, expect, it } from "vitest";
import { buildSelectedGitPatch, parseSelectableGitPatchHunk } from "./partial-line-selection";

const PATCH = [
  "diff --git a/file.txt b/file.txt",
  "index 111..222 100644",
  "--- a/file.txt",
  "+++ b/file.txt",
  "@@ -1,4 +1,4 @@ section",
  " alpha",
  "-old",
  "+new",
  " middle",
  "-last",
  "+final",
  ""
].join("\n");

describe("partial line selection", () => {
  it("parses changed lines with old and new line numbers", () => {
    const parsed = parseSelectableGitPatchHunk(PATCH);

    expect(parsed?.changedLineIds).toEqual([1, 2, 4, 5]);
    expect(parsed?.lines[1]).toMatchObject({ kind: "removed", oldLine: 2, text: "old" });
    expect(parsed?.lines[2]).toMatchObject({ kind: "added", newLine: 2, text: "new" });
  });

  it("builds a forward patch while retaining unselected deletions", () => {
    const selected = buildSelectedGitPatch(PATCH, new Set([1, 2]), "forward");

    expect(selected).toContain("@@ -1,4 +1,4 @@ section");
    expect(selected).toContain("-old\n+new");
    expect(selected).toContain(" middle\n last\n");
    expect(selected).not.toContain("+final");
  });

  it("builds a reverse patch against the complete changed version", () => {
    const selected = buildSelectedGitPatch(PATCH, new Set([1, 2]), "reverse");

    expect(selected).toContain("@@ -1,4 +1,4 @@ section");
    expect(selected).toContain("-old\n+new");
    expect(selected).toContain(" middle\n final\n");
    expect(selected).not.toContain("-last");
  });

  it("recalculates ranges for a single selected addition", () => {
    const selected = buildSelectedGitPatch(PATCH, new Set([2]), "forward");

    expect(selected).toContain("@@ -1,4 +1,5 @@ section");
    expect(selected).toContain(" old\n+new");
  });

  it("rejects file lifecycle patches and empty selections", () => {
    const newFile = PATCH.replace("index 111..222 100644", "new file mode 100644");
    expect(parseSelectableGitPatchHunk(newFile)).toBeNull();
    expect(buildSelectedGitPatch(PATCH, new Set(), "forward")).toBeNull();
  });
});
