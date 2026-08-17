import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ShellToolView } from "./shell-tool-view";

/** 转义序列起始字符 */
const ESC = String.fromCharCode(27);

/** 生成指定行数的输出文本。 */
const lines = (count: number) =>
  Array.from({ length: count }, (_, index) => `line ${index + 1}`).join("\n");

/** 渲染 Shell 视图为静态标记。 */
function render(command: string, result: Record<string, unknown>): string {
  return renderToStaticMarkup(
    <ShellToolView argumentsText={JSON.stringify({ command })} output={JSON.stringify(result)} />
  );
}

describe("ShellToolView", () => {
  it("短输出直接完整展示，不给出展开入口", () => {
    const html = render("ls", { success: true, exit_code: 0, stdout: lines(5) });
    expect(html).toContain("line 5");
    expect(html).not.toContain("collapsible-output-toggle");
    expect(html).toContain("is-scrollable");
  });

  it("长输出完整渲染并用滚动区承载，不再截断行数", () => {
    const html = render("cargo build", { success: true, exit_code: 0, stdout: lines(30) });
    expect(html).not.toContain("展开剩余");
    expect(html).not.toContain("collapsible-output-toggle");
    expect(html).toContain("is-scrollable");
    expect(html).toContain("line 12");
    expect(html).toContain("line 30");
  });

  it("解析输出中的 ANSI 着色", () => {
    const stdout = `${ESC}[31merror${ESC}[0m: failed`;
    const html = render("cargo build", { success: false, exit_code: 101, stdout });
    expect(html).toContain("ansi-fg-red");
    expect(html).toContain("error");
    expect(html).not.toContain("[31m");
  });

  it("命令失败时展示退出码", () => {
    const html = render("false", { success: false, exit_code: 1, stdout: "" });
    expect(html).toContain("shell-exit failed");
    expect(html).toContain("退出码 1");
  });

  it("命令成功时不展示退出码", () => {
    const html = render("true", { success: true, exit_code: 0, stdout: "ok" });
    expect(html).not.toContain("shell-exit");
  });

  it("有输出时命令行标记分隔态", () => {
    const html = render("ls", { success: true, exit_code: 0, stdout: "a" });
    expect(html).toContain("shell-command-line has-body");
    expect(html).toContain("language-bash");
  });

  it("无输出时命令行不加分隔线", () => {
    const html = render("touch a", { success: true, exit_code: 0, stdout: "", stderr: "" });
    expect(html).toContain('class="shell-command-line"');
  });

  it("错误输出单独成块并标记 stderr", () => {
    const html = render("cargo build", {
      success: false,
      exit_code: 1,
      stdout: "",
      stderr: "compile error"
    });
    expect(html).toContain("shell-output stderr");
    expect(html).toContain("compile error");
  });

  it("前台转后台不按失败渲染并展示去向", () => {
    const html = render("python3 -m http.server", {
      mode: "background",
      ok: true,
      task_id: "task-9",
      partial_stdout: "Serving HTTP on 127.0.0.1",
      partial_stderr: ""
    });
    expect(html).toContain("已转入后台任务");
    expect(html).toContain("task-9");
    expect(html).toContain("Serving HTTP on 127.0.0.1");
    expect(html).not.toContain("shell-exit");
    expect(html).not.toContain("退出码");
  });
});
