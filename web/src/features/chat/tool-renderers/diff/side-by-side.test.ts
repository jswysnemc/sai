import { describe, expect, it } from "vitest";
import type { DiffLine } from "./diff-model";
import { buildSideBySide } from "./side-by-side";

function line(kind: DiffLine["kind"], text: string, oldLine?: number, newLine?: number): DiffLine {
  return { kind, text, oldLine, newLine };
}

describe("buildSideBySide", () => {
  it("context 行左右同列", () => {
    const rows = buildSideBySide([line("context", "same", 1, 1)]);
    expect(rows).toEqual([{ left: line("context", "same", 1, 1), right: line("context", "same", 1, 1) }]);
  });

  it("删除与新增按位置配对同行对照", () => {
    const rows = buildSideBySide([
      line("removed", "old a", 1),
      line("added", "new a", undefined, 1)
    ]);
    expect(rows).toHaveLength(1);
    expect(rows[0].left?.text).toBe("old a");
    expect(rows[0].right?.text).toBe("new a");
  });

  it("删除多于新增时多出删除行右侧留空", () => {
    const rows = buildSideBySide([
      line("removed", "old a", 1),
      line("removed", "old b", 2),
      line("added", "new a", undefined, 1)
    ]);
    expect(rows).toHaveLength(2);
    expect(rows[0].right?.text).toBe("new a");
    expect(rows[1].left?.text).toBe("old b");
    expect(rows[1].right).toBeNull();
  });

  it("孤立新增块左侧整块留空", () => {
    const rows = buildSideBySide([
      line("context", "same", 1, 1),
      line("added", "new a", undefined, 2),
      line("added", "new b", undefined, 3)
    ]);
    expect(rows).toHaveLength(3);
    expect(rows[1].left).toBeNull();
    expect(rows[2].left).toBeNull();
    expect(rows[2].right?.text).toBe("new b");
  });

  it("hunk 标记整行保留不配对", () => {
    const rows = buildSideBySide([line("hunk", "@@ -1,2 +1,2 @@")]);
    expect(rows).toHaveLength(1);
    expect(rows[0].left?.kind).toBe("hunk");
    expect(rows[0].right).toBeNull();
  });
});
