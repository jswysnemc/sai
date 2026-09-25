import { describe, expect, it } from "vitest";
import type { DiffLine } from "../chat/tool-renderers/diff/diff-model";
import { buildEditorDiffSegments } from "./editor-diff-segments";

/** 构造测试用差异行。 */
function line(kind: DiffLine["kind"], text = "", extra: Partial<DiffLine> = {}): DiffLine {
  return { kind, text, ...extra };
}

describe("buildEditorDiffSegments", () => {
  it("把上下文和省略区间并成一条未修改间隔", () => {
    const segments = buildEditorDiffSegments([
      line("added", "new"),
      line("context", "a"),
      line("context", "b"),
      line("hunk", "@@", { foldedCount: 10 }),
      line("context", "c"),
      line("removed", "old")
    ]);
    expect(segments.map((segment) => segment.kind)).toEqual(["change", "gap", "change"]);
    expect(segments[1]).toMatchObject({ kind: "gap", count: 13 });
  });

  it("保留文件开头和结尾的未修改间隔", () => {
    const segments = buildEditorDiffSegments([
      line("hunk", "@@", { foldedCount: 4 }),
      line("added", "x"),
      line("context", "tail")
    ]);
    expect(segments.map((segment) => segment.kind)).toEqual(["gap", "change", "gap"]);
    expect(segments[0]).toMatchObject({ count: 4 });
    expect(segments[2]).toMatchObject({ count: 1 });
  });
});
