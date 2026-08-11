import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ExploreFileList } from "./explore-file-list";
import type { ToolLifecycle } from "../run-event-reducer";

/**
 * 构造测试用探索工具项。
 *
 * @param name 工具标识
 * @param args 调用参数
 * @returns 工具生命周期
 */
function tool(name: string, args: Record<string, unknown>): ToolLifecycle {
  return {
    id: `${name}-${JSON.stringify(args)}`,
    name,
    status: "completed",
    arguments: JSON.stringify(args),
    argumentsPreview: "",
    output: "",
    progress: ""
  } as ToolLifecycle;
}

/** 渲染探索清单为静态标记。 */
function render(tools: ToolLifecycle[]): string {
  return renderToStaticMarkup(<ExploreFileList tools={tools} workspacePath="/w" />);
}

describe("ExploreFileList", () => {
  it("marks shell commands with a prompt prefix", () => {
    // 命令与路径混在同一列时，只靠动作词区分需要逐行核对
    const html = render([tool("run_command", { command: "ls -la" })]);

    expect(html).toContain("is-command");
    expect(html).toContain("$");
    expect(html).toContain("ls -la");
  });

  it("keeps file paths clickable instead of turning them into commands", () => {
    const html = render([tool("read_file", { path: "/w/src/main.rs" })]);

    expect(html).not.toContain("is-command");
    expect(html).toContain("tool-file-reference");
  });

  it("collapses repeated reads of the same file into one row", () => {
    // 同一文件读两次在清单里没有额外信息，重复行只会拉长列表
    const html = render([
      tool("read_file", { path: "/w/src/main.rs" }),
      tool("read_file", { path: "/w/src/main.rs" })
    ]);

    expect(html.match(/<li /g)?.length).toBe(1);
  });

  it("keeps long commands on a single truncated row", () => {
    const long = `find . -name "*.rs" ${"-o -name '*.toml' ".repeat(20)}`;
    const html = render([tool("run_command", { command: long })]);

    // 行样式负责截断，这里确认长命令仍走同一条命令行结构而非换行渲染
    expect(html).toContain("is-command");
    expect(html).not.toContain("<br");
  });
});
