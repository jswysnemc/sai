import { describe, expect, it } from "vitest";
import type { SubagentDetail } from "../../api/contracts";
import { referencedSubagentIds, subagentIdFromOutput, subagentRecords } from "./trajectory-subagent";
import type { TrajectoryRecord } from "./trajectory-record";

/**
 * 构造一条引用了子智能体的工具记录。
 *
 * @param output 工具输出
 * @returns 轨迹记录
 */
function toolRecord(output: string): TrajectoryRecord {
  return {
    id: "t1/tool/c1",
    index: 1,
    kind: "tool",
    turnId: "t1",
    turnSeq: 1,
    turnStart: false,
    round: 1,
    roundStart: false,
    summary: "",
    label: "subagent",
    startedAt: 1000,
    durationMs: 10,
    failed: false,
    running: false,
    detail: { output }
  };
}

describe("subagentIdFromOutput", () => {
  it("从工具返回体里取出标识", () => {
    const output = JSON.stringify({ ok: true, subagent: { id: "sub_42" }, message: "started" });

    expect(subagentIdFromOutput(output)).toBe("sub_42");
  });

  it("预览被截断时退回文本匹配", () => {
    expect(subagentIdFromOutput('{"ok":true,"subagent":{"id":"sub_7","desc')).toBe("sub_7");
  });

  it("没有标识时返回 null", () => {
    expect(subagentIdFromOutput('{"ok":true}')).toBeNull();
    expect(subagentIdFromOutput(undefined)).toBeNull();
    expect(subagentIdFromOutput("   ")).toBeNull();
  });
});

describe("referencedSubagentIds", () => {
  it("只收集 subagent 工具的调用并去重", () => {
    const output = JSON.stringify({ subagent: { id: "sub_1" } });
    const other = { ...toolRecord(output), label: "read_file", id: "t1/tool/c2" };

    expect(referencedSubagentIds([toolRecord(output), toolRecord(output), other]))
      .toEqual(["sub_1"]);
  });
});

describe("subagentRecords", () => {
  /** 构造一份子智能体详情。 */
  const detail = {
    id: "sub_1",
    description: "查配置",
    subagent_type: "explore",
    status: "completed",
    max_steps: 20,
    started_at: 1000,
    updated_at: 2000,
    step: 3,
    timeline: [
      { kind: "reasoning", text: "先看配置文件" },
      { kind: "tool", step: 1, name: "read_file", args_preview: '{"path":"a.rs"}', ok: true, output_preview: "fn main" },
      { kind: "tool", step: 2, name: "grep", args_preview: '{"q":"x"}', ok: false, output_preview: "no match" },
      { kind: "text", text: "查到了" }
    ]
  } as unknown as SubagentDetail;

  it("每条时间线条目产出一条记录并挂到父调用上", () => {
    const records = subagentRecords(detail, "t1/tool/c1");

    expect(records).toHaveLength(4);
    expect(records.every((record) => record.parentId === "t1/tool/c1")).toBe(true);
    expect(records.every((record) => record.kind === "subagent")).toBe(true);
  });

  it("失败的子调用标记为失败", () => {
    const records = subagentRecords(detail, "p");

    expect(records[1].failed).toBe(false);
    expect(records[2].failed).toBe(true);
  });

  it("不虚构时刻与耗时", () => {
    // 子智能体条目只有步号没有时刻，编一个会让概览凭空多出一段耗时
    const records = subagentRecords(detail, "p");

    expect(records.every((record) => record.startedAt === null)).toBe(true);
    expect(records.every((record) => record.durationMs === null)).toBe(true);
  });

  it("工具条目带上入参与输出，思考条目走 reasoning", () => {
    const records = subagentRecords(detail, "p");

    expect(records[1].label).toBe("read_file");
    expect(records[1].detail.output).toBe("fn main");
    expect(records[0].detail.reasoning).toBe("先看配置文件");
  });
});
