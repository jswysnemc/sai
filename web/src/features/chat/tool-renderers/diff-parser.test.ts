import { describe, expect, it } from "vitest";
import { parseDiff } from "./diff-parser";

describe("diff parser", () => {
  it("解析 unified diff 的多段行号且移除 hunk 展示行", () => {
    const files = parseDiff([
      "diff --git a/src/a.ts b/src/a.ts",
      "--- a/src/a.ts",
      "+++ b/src/a.ts",
      "@@ -2,2 +2,2 @@",
      " const a = 1;",
      "-oldValue();",
      "+newValue();",
      "@@ -10 +10 @@",
      "-before();",
      "+after();"
    ].join("\n"));
    expect(files).toHaveLength(1);
    expect(files[0].lines.some((line) => line.kind === "hunk")).toBe(false);
    expect(files[0].lines.find((line) => line.text === "oldValue();")).toMatchObject({ oldLine: 3 });
    expect(files[0].lines.find((line) => line.text === "newValue();")).toMatchObject({ newLine: 3 });
    expect(files[0].lines.find((line) => line.text === "before();")).toMatchObject({ oldLine: 10 });
    expect(files[0].lines.find((line) => line.text === "after();")).toMatchObject({ newLine: 10 });
  });

  it("解析 Codex 多文件 patch 和增删统计", () => {
    const files = parseDiff([
      "*** Begin Patch",
      "*** Add File: src/new.ts",
      "+export const value = 1;",
      "*** Delete File: src/old.ts",
      "-export const old = true;",
      "*** End Patch"
    ].join("\n"));
    expect(files.map((file) => [file.path, file.action, file.added, file.removed])).toEqual([
      ["src/new.ts", "新增", 1, 0],
      ["src/old.ts", "删除", 0, 1]
    ]);
    expect(files[0].lines[0]).toMatchObject({ kind: "added", newLine: 1 });
    expect(files[1].lines[0]).toMatchObject({ kind: "removed", oldLine: 1 });
  });

  it("为没有标准 hunk 行号的 Codex 更新补充相对行号", () => {
    const files = parseDiff([
      "*** Update File: src/a.ts",
      "@@",
      "-before();",
      "+after();",
      " context();"
    ].join("\n"));
    expect(files[0].lines.find((line) => line.text === "before();")).toMatchObject({ oldLine: 1 });
    expect(files[0].lines.find((line) => line.text === "after();")).toMatchObject({ newLine: 1 });
    expect(files[0].lines.find((line) => line.text === "context();")).toMatchObject({ oldLine: 2, newLine: 2 });
  });

  it("兼容 CRLF 并移除尾部空白行", () => {
    const files = parseDiff("*** Add File: a.txt\r\n+one\r\n\r\n");
    expect(files[0].lines.at(-1)?.text).toBe("one");
  });
});
