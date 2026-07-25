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

  it("lists concrete command targets instead of a bare count", () => {
    const a = toolPart("a", "completed", "run_command", JSON.stringify({ command: "cargo test" }));
    const b = toolPart("b", "completed", "run_command", JSON.stringify({ command: "npm run typecheck" }));
    if (a.type !== "tool" || b.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([a.tool, b.tool])).toBe("执行了 cargo test、npm run typecheck");
  });

  it("summarizes read and search tools as exploration targets", () => {
    const a = toolPart("a", "completed", "read_file", JSON.stringify({ path: "src/main.rs" }));
    const b = toolPart("b", "completed", "grep", JSON.stringify({ pattern: "toolCallGroupLabel" }));
    const c = toolPart("c", "completed", "glob", JSON.stringify({ pattern: "**/*tool*" }));
    if (a.type !== "tool" || b.type !== "tool" || c.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([a.tool, b.tool, c.tool])).toBe(
      "探索了 src/main.rs、toolCallGroupLabel、**/*tool* 等"
    );
  });

  it("prefers workspace-relative paths in group labels", () => {
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
      JSON.stringify({ path: "/home/snemc/workspace/sai/src/main.rs" })
    );
    if (a.type !== "tool" || b.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([a.tool, b.tool], "zh-CN", "/home/snemc/workspace/sai")).toBe(
      "探索了 web/src/main.tsx、src/main.rs"
    );
  });

  it("does not dump raw complex regex patterns into the group title", () => {
    const messy = "执行了|个命令|ran \\d+|commands? executed|tool.?summary|explored|探索了|tool.?fold";
    const a = toolPart("a", "completed", "grep", JSON.stringify({ pattern: messy, path: "web/src" }));
    const b = toolPart("b", "completed", "run_command", JSON.stringify({ command: "semble search foo" }));
    if (a.type !== "tool" || b.type !== "tool") throw new Error("测试工具类型异常");
    const label = toolCallGroupLabel([a.tool, b.tool]);
    expect(label).toBe("探索并执行了 web/src、semble search foo");
    expect(label).not.toContain("|");
    expect(label).not.toContain("\\d");
  });

  it("falls back to a readable search label when only regex soup is available", () => {
    const messy = "执行了|个命令|ran \\d+|commands? executed|tool.?fold";
    const a = toolPart("a", "completed", "grep", JSON.stringify({ pattern: messy }));
    const b = toolPart("b", "completed", "grep", JSON.stringify({ pattern: "explored|探索了|tool.?fold" }));
    if (a.type !== "tool" || b.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([a.tool, b.tool])).toBe("探索了 代码搜索");
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
});
