import { describe, expect, it } from "vitest";
import type { LiveMessagePart, ToolLifecycle } from "../run-event-reducer";
import { groupCompletedToolCalls, toolCallGroupLabel } from "./tool-call-grouping";

/**
 * 创建指定状态的测试工具部件。
 *
 * @param id 工具标识
 * @param status 工具状态
 * @param name 工具名称
 * @returns 工具消息部件
 */
function toolPart(id: string, status: ToolLifecycle["status"], name = "run_command"): LiveMessagePart {
  return {
    id,
    type: "tool",
    tool: { id, name, status, arguments: "", argumentsPreview: "", progress: "", output: "" }
  };
}

describe("tool call grouping", () => {
  it("groups two or more adjacent completed tools", () => {
    const grouped = groupCompletedToolCalls([toolPart("a", "completed"), toolPart("b", "completed")]);
    expect(grouped).toHaveLength(1);
    expect(grouped[0].type).toBe("tool-group");
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

  it("uses a command-specific label only for command groups", () => {
    const command = toolPart("a", "completed");
    const edit = toolPart("b", "completed", "edit_file");
    if (command.type !== "tool" || edit.type !== "tool") throw new Error("测试工具类型异常");
    expect(toolCallGroupLabel([command.tool, command.tool])).toBe("运行了 2 个命令");
    expect(toolCallGroupLabel([command.tool, edit.tool])).toBe("执行了 2 项操作");
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
