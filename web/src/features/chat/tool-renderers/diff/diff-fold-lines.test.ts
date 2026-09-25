import { describe, expect, it } from "vitest";
import { contextLinesFromFile } from "./diff-fold-lines";
import type { DiffLine } from "./diff-model";

const marker: DiffLine = {
  kind: "hunk",
  text: "@@ -10,1 +10,1 @@",
  foldedCount: 3,
  foldStart: 2,
  foldEnd: 4,
  foldOldStart: 2,
  foldOldEnd: 4
};

describe("contextLinesFromFile", () => {
  it("按新文件行号回填省略的未修改行，并在两侧等长时标上旧行号", () => {
    const lines = contextLinesFromFile("a\nb\nc\nd\ne", marker);
    expect(lines.map((line) => line.text)).toEqual(["b", "c", "d"]);
    expect(lines.map((line) => line.newLine)).toEqual([2, 3, 4]);
    expect(lines.map((line) => line.oldLine)).toEqual([2, 3, 4]);
    expect(lines.every((line) => line.kind === "context")).toBe(true);
  });

  it("区间无效时不编造行", () => {
    expect(contextLinesFromFile("a", { kind: "hunk", text: "@@" })).toEqual([]);
  });
});
