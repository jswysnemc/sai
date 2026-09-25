import { describe, expect, it } from "vitest";
import type { DiffLine } from "../chat/tool-renderers/diff/diff-model";
import { hunkIndexAt } from "./editor-diff-view";

/** 构造只含类型的差异行。 */
function line(kind: DiffLine["kind"]): DiffLine {
  return { kind, text: "" };
}

describe("hunkIndexAt", () => {
  it("把省略了首个 hunk 头的正文算作第 0 个区块", () => {
    const lines = [line("context"), line("added"), line("hunk"), line("removed")];
    expect(hunkIndexAt(lines, 0, 2)).toBe(0);
    expect(hunkIndexAt(lines, 1, 2)).toBe(0);
    expect(hunkIndexAt(lines, 2, 2)).toBe(1);
    expect(hunkIndexAt(lines, 3, 2)).toBe(1);
  });

  it("把渲染出来的首个 hunk 头算作第 0 个区块", () => {
    const lines = [line("hunk"), line("added"), line("hunk"), line("context")];
    expect(hunkIndexAt(lines, 0, 2)).toBe(0);
    expect(hunkIndexAt(lines, 1, 2)).toBe(0);
    expect(hunkIndexAt(lines, 2, 2)).toBe(1);
    expect(hunkIndexAt(lines, 3, 2)).toBe(1);
  });

  it("点在行外时不返回区块", () => {
    expect(hunkIndexAt([line("added")], -1, 1)).toBe(-1);
    expect(hunkIndexAt([line("added")], 0, 0)).toBe(-1);
  });
});
