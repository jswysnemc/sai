import { describe, expect, it } from "vitest";
import { buildEditorGitLines } from "./editor-git-decorations";

/**
 * 组装单文件 unified diff 文本。
 *
 * @param body hunk 头与正文行
 * @returns 完整补丁
 */
function patch(...body: string[]): string {
  return [
    "diff --git a/src/app.ts b/src/app.ts",
    "index 1111111..2222222 100644",
    "--- a/src/app.ts",
    "+++ b/src/app.ts",
    ...body,
    ""
  ].join("\n");
}

describe("buildEditorGitLines", () => {
  it("纯新增行标记为 added", () => {
    const lines = buildEditorGitLines(patch(
      "@@ -1,2 +1,4 @@",
      " const a = 1;",
      "+const b = 2;",
      "+const c = 3;",
      " const d = 4;"
    ));

    expect(lines).toEqual([
      { line: 2, kind: "added" },
      { line: 3, kind: "added" }
    ]);
  });

  it("删改混合区段的新增侧标记为 modified", () => {
    const lines = buildEditorGitLines(patch(
      "@@ -1,3 +1,3 @@",
      " const a = 1;",
      "-const b = 2;",
      "+const b = 20;",
      " const c = 3;"
    ));

    expect(lines).toEqual([{ line: 2, kind: "modified" }]);
  });

  it("纯删除在后一行留下 deleted 锚点", () => {
    const lines = buildEditorGitLines(patch(
      "@@ -1,3 +1,2 @@",
      " const a = 1;",
      "-const b = 2;",
      " const c = 3;"
    ));

    expect(lines).toEqual([{ line: 2, kind: "deleted" }]);
  });

  it("文件末尾的删除锚在最后一个新行", () => {
    const lines = buildEditorGitLines(patch(
      "@@ -1,2 +1,1 @@",
      " const a = 1;",
      "-const b = 2;"
    ));

    expect(lines).toEqual([{ line: 1, kind: "deleted" }]);
  });

  it("多个 hunk 独立结算且结果按行号排序", () => {
    const lines = buildEditorGitLines(patch(
      "@@ -10,2 +10,3 @@",
      " context;",
      "+added;",
      " tail;",
      "@@ -30,2 +31,2 @@",
      "-old;",
      "+new;",
      " tail2;"
    ));

    expect(lines).toEqual([
      { line: 11, kind: "added" },
      { line: 31, kind: "modified" }
    ]);
  });

  it("空补丁返回空装饰", () => {
    expect(buildEditorGitLines("")).toEqual([]);
  });
});
