import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ToolLifecycleCard } from "../tool-lifecycle-card";
import type { ToolLifecycle } from "../run-event-reducer";

/** 构造一项工具生命周期，未给出的字段取空默认值。 */
function makeTool(patch: Partial<ToolLifecycle>): ToolLifecycle {
  return {
    id: "tool-1",
    name: "run_command",
    argumentsPreview: "",
    arguments: "",
    progress: "",
    output: "",
    status: "completed",
    ...patch
  };
}

/** 在查询上下文中渲染工具卡为静态标记。 */
function render(tool: ToolLifecycle): string {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return renderToStaticMarkup(
    <QueryClientProvider client={queryClient}>
      <ToolLifecycleCard tool={tool} />
    </QueryClientProvider>
  );
}

describe("ToolLifecycleCard 折叠行", () => {
  it("已结束的调用在头部展示耗时", () => {
    const html = render(makeTool({
      arguments: JSON.stringify({ command: "cargo test" }),
      output: JSON.stringify({ success: true, exit_code: 0, stdout: "ok" }),
      startedAtMs: 1_000,
      endedAtMs: 2_500
    }));
    expect(html).toContain("1.5s");
  });

  it("耗时过短的调用不展示耗时", () => {
    const html = render(makeTool({
      arguments: JSON.stringify({ command: "true" }),
      output: JSON.stringify({ success: true, exit_code: 0, stdout: "" }),
      startedAtMs: 1_000,
      endedAtMs: 1_050
    }));
    expect(html).not.toContain("tool-shell-meta");
  });

  it("命令失败时折叠行给出危险色退出码", () => {
    const html = render(makeTool({
      name: "run_command",
      arguments: JSON.stringify({ command: "false" }),
      output: JSON.stringify({ success: false, exit_code: 1, stdout: "" }),
      status: "failed"
    }));
    expect(html).toContain("tool-shell-summary is-danger");
    expect(html).toContain("退出码 1");
  });

  it("读取类调用折叠行给出行数", () => {
    const html = render(makeTool({
      name: "read_file",
      arguments: JSON.stringify({ path: "/repo/src/main.rs" }),
      output: JSON.stringify({ type: "text-page", path: "/repo/src/main.rs", content: "1: a\n2: b" })
    }));
    expect(html).toContain("2 行");
  });

  it("编辑类调用折叠行分别展示增删", () => {
    const html = render(makeTool({
      name: "write_file",
      arguments: JSON.stringify({ path: "/repo/a.rs", content: "x" }),
      output: JSON.stringify({ changed_files: [{ path: "/repo/a.rs", added: 4, removed: 2 }] })
    }));
    expect(html).toContain("+4");
    expect(html).toContain("-2");
  });

  it("无结果可述时不渲染摘要位", () => {
    const html = render(makeTool({
      name: "some_unknown_tool",
      arguments: JSON.stringify({ query: "x" }),
      output: "done"
    }));
    expect(html).not.toContain("tool-shell-summary");
  });

  it("头部提供复制调用内容与复制输出的操作", () => {
    const html = render(makeTool({
      arguments: JSON.stringify({ command: "ls" }),
      output: JSON.stringify({ success: true, exit_code: 0, stdout: "a" })
    }));
    expect(html).toContain("复制调用内容");
    expect(html).toContain("复制输出");
  });

  it("没有输出时不提供复制输出操作", () => {
    const html = render(makeTool({
      arguments: JSON.stringify({ command: "ls" }),
      output: "",
      status: "running"
    }));
    expect(html).not.toContain("复制输出");
  });

  it("折叠态展示完整命令而非压缩摘要", () => {
    const command = "node /home/snemc/workspace/tmp/sandbox/ball-battle/test/smoke-test.mjs --reporter verbose";
    const html = render(makeTool({
      arguments: JSON.stringify({ command }),
      output: JSON.stringify({ success: true, exit_code: 0, stdout: "ok" })
    }));
    expect(html).toContain(command);
  });

  it("展开的命令卡头部不再重复命令", () => {
    const command = "cargo test --workspace";
    // 失败的卡片默认展开，头部命令应让位给详情区的 $ 命令行
    const html = render(makeTool({
      id: "expanded-shell",
      arguments: JSON.stringify({ command }),
      output: JSON.stringify({ success: false, exit_code: 1, stdout: "" }),
      status: "failed"
    }));
    const occurrences = html.split(command).length - 1;
    expect(occurrences).toBe(1);
    expect(html).toContain("shell-command-line");
  });

  it("流式写入期间折叠行展示已写入行数", () => {
    const html = render(makeTool({
      id: "writing",
      name: "write_file",
      argumentsPreview: '{"path":"a.rs","content":"l1\\nl2\\nl3\\nl4',
      arguments: "",
      status: "running"
    }));
    expect(html).toContain("写入");
    expect(html).toContain("行");
  });
});
