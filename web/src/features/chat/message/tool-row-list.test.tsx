import { renderToStaticMarkup } from "react-dom/server";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { describe, expect, it } from "vitest";
import { ToolRowList } from "./tool-row-list";
import type { ToolLifecycle } from "../run-event-reducer";

/**
 * 构造测试用工具项。
 *
 * @param name 工具标识
 * @param args 调用参数
 * @param output 工具输出
 * @returns 工具生命周期
 */
function tool(name: string, args: Record<string, unknown>, output = ""): ToolLifecycle {
  return {
    id: `${name}-${JSON.stringify(args)}`,
    name,
    status: "completed",
    arguments: JSON.stringify(args),
    argumentsPreview: "",
    output,
    progress: ""
  } as ToolLifecycle;
}

/** 渲染清单为静态标记。 */
function render(tools: ToolLifecycle[]): string {
  const client = new QueryClient();
  return renderToStaticMarkup(
    <QueryClientProvider client={client}>
      <ToolRowList tools={tools} workspacePath="/w" />
    </QueryClientProvider>
  );
}

describe("ToolRowList", () => {
  it("renders done verbs with monospace commands", () => {
    const html = render([tool("run_command", { command: "ls -la" })]);

    expect(html).toContain("已执行");
    expect(html).toContain("is-command");
    expect(html).toContain("ls -la");
  });

  it("splits file rows into clickable name and muted directory", () => {
    const html = render([tool("read_file", { path: "/w/docs/README.md" })]);

    expect(html).toContain("已读取");
    expect(html).toContain("tool-file-reference");
    expect(html).toContain("README.md");
    // 父目录作为弱化后缀单独展示，扫视时先看到文件名
    expect(html).toContain("tool-row-directory");
    expect(html).toContain("docs/");
  });

  it("collapses repeated reads of the same file into one row", () => {
    const html = render([
      tool("read_file", { path: "/w/src/main.rs" }),
      tool("read_file", { path: "/w/src/main.rs" })
    ]);

    expect(html.match(/<li /g)?.length).toBe(1);
  });

  it("marks rows with output as expandable", () => {
    const html = render([tool("run_command", { command: "ls" }, "file-a\nfile-b")]);

    // 有输出的命令行是按钮并带展开箭头，可单独展开详情
    expect(html).toContain("button");
    expect(html).toContain("tool-row-chevron");
    expect(html).toContain("aria-expanded");
  });

  it("keeps pure file reads as plain rows without expanders", () => {
    const html = render([tool("read_file", { path: "/w/a.rs" }, "fn main() {}")]);

    expect(html).not.toContain("tool-row-chevron");
  });
});
