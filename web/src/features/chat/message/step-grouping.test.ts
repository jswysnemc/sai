import { describe, expect, it } from "vitest";
import type { LiveMessagePart, ToolLifecycle } from "../run-event-reducer";
import { groupIntoSteps, type StepGroup } from "./step-grouping";

/**
 * 创建指定状态的测试工具部件。
 *
 * @param id 工具标识
 * @param status 工具状态
 * @returns 工具消息部件
 */
function toolPart(id: string, status: ToolLifecycle["status"] = "completed"): LiveMessagePart {
  return {
    id,
    type: "tool",
    tool: {
      id,
      name: "run_command",
      status,
      arguments: "{}",
      argumentsPreview: "{}",
      progress: "",
      output: ""
    }
  };
}

/**
 * 创建思考部件。
 *
 * @param id 部件标识
 * @param ended 思考是否已结束
 * @returns 思考消息部件
 */
function reasoningPart(id: string, ended = true): LiveMessagePart {
  return {
    id,
    type: "reasoning",
    source: `thinking ${id}`,
    startedAt: "1",
    ...(ended ? { endedAt: "2" } : {})
  };
}

/**
 * 创建正文部件。
 *
 * @param id 部件标识
 * @returns 正文消息部件
 */
function textPart(id: string): LiveMessagePart {
  return { id, type: "text", source: "answer" };
}

describe("step grouping", () => {
  it("merges a reasoning block with the tools that follow it", () => {
    const grouped = groupIntoSteps([
      reasoningPart("r1"),
      toolPart("t1"),
      toolPart("t2")
    ]);

    expect(grouped).toHaveLength(1);
    const step = grouped[0] as StepGroup;
    expect(step.type).toBe("step");
    expect(step.reasoning.id).toBe("r1");
    expect(step.tools.map((tool) => tool.id)).toEqual(["t1", "t2"]);
  });

  it("keeps alternating rounds as separate steps", () => {
    const grouped = groupIntoSteps([
      reasoningPart("r1"),
      toolPart("t1"),
      reasoningPart("r2"),
      toolPart("t2"),
      reasoningPart("r3"),
      toolPart("t3")
    ]);

    // 交替场景正是纵向空间失控的来源：三轮应折成三个步骤，而不是六个部件
    expect(grouped).toHaveLength(3);
    expect(grouped.every((item) => item.type === "step")).toBe(true);
  });

  it("treats assistant text as a step boundary", () => {
    const grouped = groupIntoSteps([
      reasoningPart("r1"),
      toolPart("t1"),
      textPart("x1"),
      reasoningPart("r2"),
      toolPart("t2")
    ]);

    expect(grouped.map((item) => item.type)).toEqual(["step", "part", "step"]);
  });

  it("leaves an unfinished reasoning block on its own", () => {
    const grouped = groupIntoSteps([reasoningPart("r1", false), toolPart("t1")]);

    // 本步骤仍在进行，思考要保持实时可见
    expect(grouped[0].type).toBe("part");
    expect(grouped.some((item) => item.type === "step")).toBe(false);
  });

  it("does not absorb running or failed tools", () => {
    const grouped = groupIntoSteps([
      reasoningPart("r1"),
      toolPart("t1", "running"),
      toolPart("t2", "failed")
    ]);

    // 未完成的工具需要独立展示，因此该思考不成步骤
    expect(grouped.some((item) => item.type === "step")).toBe(false);
    expect(grouped).toHaveLength(3);
  });

  it("falls back to adjacent tool grouping without reasoning", () => {
    const grouped = groupIntoSteps([toolPart("t1"), toolPart("t2")]);

    expect(grouped).toHaveLength(1);
    expect(grouped[0].type).toBe("tool-group");
  });

  it("keeps a lone reasoning block ungrouped", () => {
    const grouped = groupIntoSteps([reasoningPart("r1"), textPart("x1")]);

    expect(grouped.map((item) => item.type)).toEqual(["part", "part"]);
  });

  it("returns an empty list unchanged", () => {
    expect(groupIntoSteps([])).toEqual([]);
  });
});
