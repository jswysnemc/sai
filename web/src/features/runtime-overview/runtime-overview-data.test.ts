import { describe, expect, it } from "vitest";
import { countGitPatchLines, normalizeTodoItems } from "./runtime-overview-data";

describe("runtime overview data", () => {
  it("counts changed lines without diff headers", () => {
    const patch = [
      "--- a/file.ts",
      "+++ b/file.ts",
      "@@ -1,2 +1,3 @@",
      "-old",
      "+new",
      "+added"
    ].join("\n");

    expect(countGitPatchLines(patch)).toEqual({ added: 2, removed: 1 });
  });

  it("normalizes current and legacy todo responses", () => {
    const item = { id: "todo-1", text: "检查结果", status: "pending" as const, created_at: "", updated_at: "" };

    expect(normalizeTodoItems([item])).toEqual([item]);
    expect(normalizeTodoItems({ items: [item], history: [] })).toEqual([item]);
  });
});
