import { describe, expect, it } from "vitest";
import { toolDiffStat, toolResultSummary } from "./tool-result-summary";

describe("toolResultSummary", () => {
  it("读取单文件时给出行数", () => {
    const output = JSON.stringify({
      type: "text-page",
      path: "src/main.rs",
      content: "1: fn main() {\n2: }"
    });
    expect(toolResultSummary("read_file", output)).toEqual({ label: "2 行", tone: "neutral" });
  });

  it("批量读取补上文件数", () => {
    const output = JSON.stringify({
      type: "multi-text-page",
      results: [
        { type: "text-page", path: "a.rs", content: "1: a" },
        { type: "text-page", path: "b.rs", content: "1: b\n2: c" }
      ]
    });
    expect(toolResultSummary("read_file", output)).toEqual({
      label: "2 个文件 · 3 行",
      tone: "neutral"
    });
  });

  it("搜索按命中行数统计匹配", () => {
    const output = JSON.stringify({ stdout: "a.rs:1:x\nb.rs:2:y", truncated: false });
    expect(toolResultSummary("grep", output)).toEqual({ label: "2 处匹配", tone: "neutral" });
  });

  it("搜索结果被截断时标注加号", () => {
    const output = JSON.stringify({ stdout: "a.rs:1:x", truncated: true });
    expect(toolResultSummary("grep", output)?.label).toBe("1+ 处匹配");
  });

  it("零命中使用后端给出的 matches 字段", () => {
    const output = JSON.stringify({ stdout: "", matches: 0 });
    expect(toolResultSummary("grep", output)).toEqual({ label: "无匹配", tone: "neutral" });
  });

  it("命令成功时不占用摘要位", () => {
    const output = JSON.stringify({ success: true, exit_code: 0, stdout: "ok" });
    expect(toolResultSummary("run_command", output)).toBeNull();
  });

  it("命令失败时给出危险色退出码", () => {
    const output = JSON.stringify({ success: false, exit_code: 127, stdout: "" });
    expect(toolResultSummary("run_command", output)).toEqual({
      label: "退出码 127",
      tone: "danger"
    });
  });

  it("前台转后台的提升按中性去向摘要，不算失败", () => {
    const output = JSON.stringify({
      mode: "background",
      ok: true,
      task_id: "task-1",
      partial_stdout: ""
    });
    expect(toolResultSummary("run_command", output)).toEqual({
      label: "已转入后台",
      tone: "neutral"
    });
  });

  it("英文界面下使用英文摘要", () => {
    const output = JSON.stringify({ stdout: "a.rs:1:x", truncated: false });
    expect(toolResultSummary("grep", output, "en-US")?.label).toBe("1 matches");
  });

  it("空输出返回空摘要", () => {
    expect(toolResultSummary("read_file", "")).toBeNull();
  });
});

describe("toolDiffStat", () => {
  it("汇总多文件增删行数", () => {
    const output = JSON.stringify({
      changed_files: [
        { path: "a.rs", added: 3, removed: 1 },
        { path: "b.rs", added: 2, removed: 0 }
      ]
    });
    expect(toolDiffStat("edit_file", output)).toEqual({ added: 5, removed: 1 });
  });

  it("非编辑类工具不返回增删", () => {
    const output = JSON.stringify({ changed_files: [{ added: 1, removed: 1 }] });
    expect(toolDiffStat("read_file", output)).toBeNull();
  });

  it("全为零时不返回统计", () => {
    const output = JSON.stringify({ changed_files: [{ added: 0, removed: 0 }] });
    expect(toolDiffStat("edit_file", output)).toBeNull();
  });
});
