import { describe, expect, it } from "vitest";
import type { LiveMessagePart, ToolLifecycle } from "../run-event-reducer";
import { groupCompletedToolCalls, toolCallGroupLabel } from "./tool-call-grouping";

/**
 * 创建指定状态的测试工具部件。
 *
 * @param id 工具标识
 * @param status 工具状态
 * @param name 工具名称
 * @param args 参数 JSON
 * @returns 工具消息部件
 */
function toolPart(
  id: string,
  status: ToolLifecycle["status"],
  name = "run_command",
  args = "{}"
): LiveMessagePart {
  return {
    id,
    type: "tool",
    tool: {
      id,
      name,
      status,
      arguments: args,
      argumentsPreview: args,
      progress: "",
      output: ""
    }
  };
}

describe("tool call grouping", () => {
  it("groups two or more adjacent completed tools", () => {
    const grouped = groupCompletedToolCalls([toolPart("a", "completed"), toolPart("b", "completed")]);
    expect(grouped).toHaveLength(1);
    expect(grouped[0].type).toBe("tool-group");
  });

  it("keeps a stable group id when more tools append", () => {
    const first = groupCompletedToolCalls([toolPart("a", "completed"), toolPart("b", "completed")]);
    const second = groupCompletedToolCalls([
      toolPart("a", "completed"),
      toolPart("b", "completed"),
      toolPart("c", "completed")
    ]);
    expect(first[0].id).toBe("tool-group-a");
    expect(second[0].id).toBe("tool-group-a");
  });

  it("keeps running and failed tools visible outside groups", () => {
    const grouped = groupCompletedToolCalls([
      toolPart("a", "completed"),
      toolPart("b", "completed"),
      toolPart("c", "running"),
      toolPart("d", "failed"),
      toolPart("e", "completed")
    ]);
    expect(grouped.map((item) => item.type)).toEqual(["tool-group", "part", "part", "part"]);
    expect(grouped.filter((item) => item.type === "part").map((item) => item.part.id)).toEqual(["c", "d", "e"]);
  });

  it("does not group tools across text boundaries", () => {
    const text: LiveMessagePart = { id: "text", type: "text", source: "说明" };
    const grouped = groupCompletedToolCalls([toolPart("a", "completed"), text, toolPart("b", "completed")]);
    expect(grouped.map((item) => item.type)).toEqual(["part", "part", "part"]);
  });

  it("counts commands instead of listing raw targets", () => {
    const a = toolPart("a", "completed", "run_command", JSON.stringify({ command: "cargo test" }));
    const b = toolPart("b", "completed", "run_command", JSON.stringify({ command: "npm run typecheck" }));
    if (a.type !== "tool" || b.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([a.tool, b.tool])).toBe("运行了 2 个命令");
  });

  it("counts read and search tools as explored files", () => {
    const a = toolPart("a", "completed", "read_file", JSON.stringify({ path: "src/main.rs" }));
    const b = toolPart("b", "completed", "grep", JSON.stringify({ pattern: "toolCallGroupLabel" }));
    const c = toolPart("c", "completed", "glob", JSON.stringify({ pattern: "**/*tool*" }));
    if (a.type !== "tool" || b.type !== "tool" || c.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([a.tool, b.tool, c.tool])).toBe("探索了 3 个文件");
  });

  it("dedupes repeated reads of the same file", () => {
    const a = toolPart(
      "a",
      "completed",
      "read_file",
      JSON.stringify({ path: "/home/snemc/workspace/sai/web/src/main.tsx" })
    );
    const b = toolPart(
      "b",
      "completed",
      "read_file",
      JSON.stringify({ path: "/home/snemc/workspace/sai/web/src/main.tsx" })
    );
    const c = toolPart(
      "c",
      "completed",
      "read_file",
      JSON.stringify({ path: "/home/snemc/workspace/sai/src/main.rs" })
    );
    if (a.type !== "tool" || b.type !== "tool" || c.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([a.tool, b.tool, c.tool], "zh-CN", "/home/snemc/workspace/sai")).toBe(
      "探索了 2 个文件"
    );
  });

  it("treats read-only shell commands as exploration", () => {
    const a = toolPart("a", "completed", "run_command", JSON.stringify({ command: "cat web/src/main.tsx" }));
    const b = toolPart("b", "completed", "run_command", JSON.stringify({ command: "ls -la src" }));
    if (a.type !== "tool" || b.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([a.tool, b.tool])).toBe("探索了 2 个文件");
  });

  it("keeps mutating commands in the command bucket alongside reads", () => {
    const read = toolPart("a", "completed", "run_command", JSON.stringify({ command: "git status" }));
    const write = toolPart("b", "completed", "run_command", JSON.stringify({ command: "cargo build" }));
    const file = toolPart("c", "completed", "read_file", JSON.stringify({ path: "src/main.rs" }));
    if (read.type !== "tool" || write.type !== "tool" || file.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([read.tool, write.tool, file.tool])).toBe("探索了 2 个文件，运行了 1 个命令");
  });

  it("does not dump raw complex regex patterns into the group title", () => {
    const messy = "执行了|个命令|ran \\d+|commands? executed|tool.?summary|explored|探索了|tool.?fold";
    const a = toolPart("a", "completed", "grep", JSON.stringify({ pattern: messy, path: "web/src" }));
    const b = toolPart("b", "completed", "run_command", JSON.stringify({ command: "semble search foo" }));
    if (a.type !== "tool" || b.type !== "tool") throw new Error("测试工具类型异常");
    const label = toolCallGroupLabel([a.tool, b.tool]);
    expect(label).toBe("探索了 1 个文件，运行了 1 个命令");
    expect(label).not.toContain("|");
    expect(label).not.toContain("\\d");
  });

  it("counts unresolvable searches by call when only regex soup is available", () => {
    const messy = "执行了|个命令|ran \\d+|commands? executed|tool.?fold";
    const a = toolPart("a", "completed", "grep", JSON.stringify({ pattern: messy }));
    const b = toolPart("b", "completed", "grep", JSON.stringify({ pattern: "explored|探索了|tool.?fold" }));
    if (a.type !== "tool" || b.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([a.tool, b.tool])).toBe("探索了 1 个文件");
  });

  it("groups consecutive completed todo calls with a plan label", () => {
    const grouped = groupCompletedToolCalls([
      toolPart("a", "completed", "todo"),
      toolPart("b", "completed", "todo"),
      toolPart("c", "completed", "todo")
    ]);
    expect(grouped).toHaveLength(1);
    expect(grouped[0].type).toBe("tool-group");
    if (grouped[0].type !== "tool-group") throw new Error("测试分组类型异常");
    expect(toolCallGroupLabel(grouped[0].tools)).toBe("更新了 3 次计划");
  });

  it("keeps other tools in their own bucket next to read-only commands", () => {
    const long = {
      id: "t1",
      name: "subagent",
      arguments: JSON.stringify({
        task: "测试 subagent 工具 - 读取并汇报文件内容，另外还要顺便检查若干细节"
      }),
      argumentsPreview: "",
      progress: "",
      output: "",
      status: "completed" as const
    };
    const short = {
      ...long,
      id: "t2",
      name: "run_command",
      arguments: JSON.stringify({ command: "git status" })
    };

    const label = toolCallGroupLabel([long, short]);

    // 只读的 git status 归入探索桶，subagent 落在其他工具桶
    expect(label).toBe("探索了 1 个文件，执行了 1 个工具");
  });
});
