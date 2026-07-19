import { describe, expect, it } from "vitest";
import { createFileComparisonTarget, selectFileComparisonBase } from "./file-comparison-state";

describe("file comparison state", () => {
  it("keeps comparison bases isolated by repository", () => {
    const first = selectFileComparisonBase({}, "/repo/a", "first.ts");
    const second = selectFileComparisonBase(first, "/repo/b", "second.ts");
    expect(second).toEqual({ "/repo/a": "first.ts", "/repo/b": "second.ts" });
  });

  it("creates comparisons only for different files in one repository", () => {
    const bases = { "/repo/a": "first.ts" };
    expect(createFileComparisonTarget(bases, "/repo/a", "second.ts")).toEqual({
      repoRoot: "/repo/a",
      basePath: "first.ts",
      headPath: "second.ts"
    });
    expect(createFileComparisonTarget(bases, "/repo/a", "first.ts")).toBeNull();
    expect(createFileComparisonTarget(bases, "/repo/b", "second.ts")).toBeNull();
  });
});
