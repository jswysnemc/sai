import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ToolResultView } from "./tool-result-view";
import { ToolIcon } from "./tool-icon";
import { isEditToolName } from "./tool-display-summary";
import { toolDiffStat } from "./tool-result-summary";
import { toolFilePath } from "./tool-data";
import { streamedDiffCounts } from "./write-progress";
import { PermissionArgumentDetails } from "../../permission/permission-argument-details";
import { collectTurnFileChanges } from "../turn-changes/collect-turn-file-changes";
import { buildTurnDiffSource } from "../turn-changes/build-turn-diff-source";
import { parseDiff } from "./diff/diff-parser";

const patch = "*** Begin Patch\n*** Update File: one.ts\n@@\n-old\n+new\n*** Add File: two.ts\n+second\n*** End Patch";
const args = JSON.stringify({ patch });
const diff = "diff --git a/one.ts b/one.ts\n--- a/one.ts\n+++ b/one.ts\n@@ -1 +1 @@\n-old\n+new\ndiff --git a/two.ts b/two.ts\n--- /dev/null\n+++ b/two.ts\n@@ -0,0 +1 @@\n+second\n";
const output = JSON.stringify({ ok: true, changed_files: [
  { path: "one.ts", action: "Edited", added: 1, removed: 1 },
  { path: "two.ts", action: "Added", added: 1, removed: 0 },
], diff });

describe("apply_patch integration", () => {
  it("复用编辑图标、真实差异和多文件增删统计", () => {
    expect(isEditToolName("apply_patch")).toBe(true);
    expect(toolDiffStat("apply_patch", output)).toEqual({ added: 2, removed: 1 });
    expect(toolFilePath("apply_patch", args)).toBe("");
    const icon = renderToStaticMarkup(<ToolIcon name="apply_patch" />);
    expect(icon).toBe(renderToStaticMarkup(<ToolIcon name="str_replace" />));
    const html = renderToStaticMarkup(<ToolResultView name="apply_patch" argumentsText={args} output={output} />);
    expect(html).toContain("structured-diff");
    expect(html).toContain("one.ts");
    expect(html).toContain("two.ts");
    expect(html).toContain("diff-unified-row added");
    expect(html).toContain("diff-unified-row removed");
  });

  it("审批显示计划差异，参数流显示补丁增删进度", () => {
    const html = renderToStaticMarkup(<PermissionArgumentDetails tool="apply_patch" argumentsText={args} />);
    expect(html).toContain("structured-diff");
    expect(streamedDiffCounts(args)).toEqual({ added: 2, removed: 1 });
    expect(streamedDiffCounts('{"patch":"*** Begin Patch\\n*** Update File: a\\n@@\\n-old\\n+new')).toEqual({ added: 1, removed: 1 });
  });

  it("成功变更进入本轮文件列表，单文件差异不混入其他文件", () => {
    const tool = { name: "apply_patch", arguments: args, output, status: "completed", ok: true };
    expect(collectTurnFileChanges([tool]).map((file) => file.path)).toEqual(["one.ts", "two.ts"]);
    const selected = buildTurnDiffSource([tool], "one.ts");
    expect(selected).toContain("+new");
    expect(selected).not.toContain("two.ts");
  });

  it("失败或缺少执行报告时不把计划计为实际变更，并显示错误原因", () => {
    const error = "tool error: patch failed; ROLLBACK INCOMPLETE: one.ts";
    const failed = { name: "apply_patch", arguments: args, output: error, status: "completed", ok: false };
    expect(collectTurnFileChanges([failed])).toEqual([]);
    expect(buildTurnDiffSource([failed], null)).toBe("");
    expect(collectTurnFileChanges([{ ...failed, output: "", ok: true }])).toEqual([]);
    expect(buildTurnDiffSource([{ ...failed, output: "", ok: true }], null)).toBe("");
    const html = renderToStaticMarkup(<ToolResultView name="apply_patch" argumentsText={args} output={error} />);
    expect(html).toContain("ROLLBACK INCOMPLETE");
    expect(html).toContain("tool-error-output");
  });

  it("移动预览使用目标路径，文件末尾标记不成为代码行", () => {
    const move = "*** Begin Patch\n*** Update File: old.ts\n*** Move to: new.ts\n*** End Patch";
    expect(toolFilePath("apply_patch", JSON.stringify({ patch: move }))).toBe("new.ts");
    expect(parseDiff(move)).toMatchObject([{ path: "new.ts", oldPath: "old.ts", status: "renamed", lines: [] }]);
    const files = parseDiff(patch.replace("*** Add File: two.ts", "*** End of File\n*** Add File: two.ts"));
    expect(files.flatMap((file) => file.lines).some((line) => line.text.includes("End of File"))).toBe(false);
  });

  it("空文件新增和删除保留文件块，清空原文件不误标为删除", () => {
    const empty = "diff --git a/added b/added\n--- /dev/null\n+++ b/added\ndiff --git a/deleted b/deleted\n--- a/deleted\n+++ /dev/null\n";
    expect(parseDiff(empty)).toMatchObject([
      { path: "added", status: "added", lines: [] },
      { path: "deleted", status: "deleted", lines: [] },
    ]);
    const cleared = "diff --git a/cleared b/cleared\n--- a/cleared\n+++ b/cleared\n@@ -1 +0,0 @@\n-old\n";
    expect(parseDiff(cleared)).toMatchObject([{ path: "cleared", status: "modified", removed: 1 }]);
  });
});
