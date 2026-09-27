import { describe, expect, it } from "vitest";
import type { DiffLine } from "../chat/tool-renderers/diff/diff-model";
import { buildEditorDiffSegments } from "./editor-diff-segments";

/** 构造测试用差异行。 */
function line(kind: DiffLine["kind"], text = "", extra: Partial<DiffLine> = {}): DiffLine {
  return { kind, text, ...extra };
}

describe("buildEditorDiffSegments", () => {
  it("改动两侧各留三行，中间的省略区间继续折叠", () => {
    const segments = buildEditorDiffSegments([
      line("added", "new"),
      line("context", "a"),
      line("context", "b"),
      line("context", "c"),
      line("hunk", "@@", { foldedCount: 10 }),
      line("context", "d"),
      line("context", "e"),
      line("context", "f"),
      line("removed", "old")
    ]);
    expect(segments.map((segment) => segment.kind)).toEqual(["change", "context", "gap", "context", "change"]);
    expect(segments[1]).toMatchObject({ kind: "context", lines: [{ line: { text: "a" } }, { line: { text: "b" } }, { line: { text: "c" } }] });
    expect(segments[2]).toMatchObject({ kind: "gap", count: 10 });
    expect(segments[3]).toMatchObject({ kind: "context", lines: [{ line: { text: "d" } }, { line: { text: "e" } }, { line: { text: "f" } }] });
  });

  it("不到六行的未修改内容直接展开", () => {
    const segments = buildEditorDiffSegments([
      line("context", "above-1"),
      line("context", "above-2"),
      line("added", "x"),
      line("context", "tail")
    ]);
    expect(segments.map((segment) => segment.kind)).toEqual(["context", "change", "context"]);
    expect(segments[0]).toMatchObject({ lines: [{ line: { text: "above-1" } }, { line: { text: "above-2" } }] });
    expect(segments[2]).toMatchObject({ lines: [{ line: { text: "tail" } }] });
  });

  it("文件开头的省略区间留在折叠条里，紧贴改动的行仍然可见", () => {
    const segments = buildEditorDiffSegments([
      line("hunk", "@@", { foldedCount: 4 }),
      line("context", "near"),
      line("added", "x")
    ]);
    expect(segments.map((segment) => segment.kind)).toEqual(["gap", "context", "change"]);
    expect(segments[0]).toMatchObject({ count: 4 });
    expect(segments[1]).toMatchObject({ lines: [{ line: { text: "near" } }] });
  });
});
