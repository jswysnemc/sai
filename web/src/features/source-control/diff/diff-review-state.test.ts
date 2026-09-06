import { describe, expect, it } from "vitest";
import type { DiffFile } from "../../chat/tool-renderers/diff/diff-model";
import { isDiffFileCollapsed } from "./diff-review-state";

const largeFile: DiffFile = {
  path: "large.ts", status: "modified", added: 301, removed: 0,
  lines: Array.from({ length: 301 }, (_, index) => ({ kind: "added", newLine: index + 1, text: "content" })),
};

describe("diff file collapse state", () => {
  it("大文件在初次渲染前就折叠，选中的大文件则直接展开", () => {
    expect(isDiffFileCollapsed(largeFile, new Map(), null)).toBe(true);
    expect(isDiffFileCollapsed(largeFile, new Map(), largeFile.path)).toBe(false);
  });
  it("后续数据刷新保留用户明确设置的状态", () => {
    expect(isDiffFileCollapsed(largeFile, new Map([[largeFile.path, false]]), null)).toBe(false);
    expect(isDiffFileCollapsed(largeFile, new Map([[largeFile.path, true]]), largeFile.path)).toBe(true);
  });
});
