import { describe, expect, it } from "vitest";
import type { SessionTimeline, SessionTimelineTurn, TimelineToolEntry } from "../../api/contracts";
import { buildTrajectory } from "./trajectory-build";

/**
 * 构造一条测试用的工具调用。
 *
 * @param overrides 需要覆盖的字段
 * @returns 时间线工具条目
 */
function tool(overrides: Partial<TimelineToolEntry> & { id: string }): TimelineToolEntry {
  return {
    name: "read_file",
    arguments: "{}",
    status: "completed",
    output: "",
    created_at: "2026-08-14T10:00:00Z",
    ...overrides
  };
}

/**
 * 构造一条测试用的轮次。
 *
 * @param overrides 需要覆盖的字段
 * @returns 时间线轮次
 */
function turn(overrides: Partial<SessionTimelineTurn> & { turn_id: string }): SessionTimelineTurn {
  return {
    seq: 1,
    status: "completed",
    automatic: false,
    user: { timestamp: "2026-08-14T10:00:00Z", content: "查一下配置" },
    assistant: { timestamp: "2026-08-14T10:00:10Z", content: "已查到" },
    tools: [],
    ...overrides
  };
}

/**
 * 构造只含给定轮次的时间线。
 *
 * @param turns 轮次列表
 * @returns 会话时间线
 */
function timeline(turns: SessionTimelineTurn[]): SessionTimeline {
  return { turns };
}

describe("buildTrajectory", () => {
  it("按用户输入、工具、助手正文的顺序展开一轮", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        tools: [tool({ id: "c1", seq: 1, assistant_round: 1 })]
      })
    ]));

    expect(model.records.map((record) => record.kind)).toEqual(["user", "tool", "assistant"]);
    expect(model.records.map((record) => record.index)).toEqual([1, 2, 3]);
    expect(model.records[0].turnStart).toBe(true);
  });

  it("把同一 assistant_round 的工具归入同一次请求", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        tools: [
          tool({ id: "c1", seq: 1, assistant_round: 1 }),
          tool({ id: "c2", seq: 2, assistant_round: 1 }),
          tool({ id: "c3", seq: 3, assistant_round: 3 })
        ]
      })
    ]));

    const tools = model.records.filter((record) => record.kind === "tool");
    expect(tools.map((record) => record.round)).toEqual([1, 1, 2]);
    expect(tools.map((record) => record.roundStart)).toEqual([true, false, true]);
    expect(model.turns[0].requestCount).toBe(3);
  });

  it("从工具的起止时刻算出自身耗时", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        tools: [tool({
          id: "c1",
          seq: 1,
          assistant_round: 1,
          created_at: "2026-08-14T10:00:00.000Z",
          completed_at: "2026-08-14T10:00:01.500Z"
        })]
      })
    ]));

    expect(model.records.find((record) => record.kind === "tool")?.durationMs).toBe(1500);
  });

  it("未完成的工具不虚构耗时", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        status: "running",
        tools: [tool({ id: "c1", seq: 1, assistant_round: 1, status: "running", completed_at: null })]
      })
    ]));

    const running = model.records.find((record) => record.kind === "tool");
    expect(running?.durationMs).toBeNull();
    expect(running?.running).toBe(true);
  });

  it("把插入消息排在触发它的工具之后", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        tools: [
          tool({ id: "c1", seq: 1, assistant_round: 1 }),
          tool({ id: "c2", seq: 2, assistant_round: 1 })
        ],
        messages: [{
          id: "m1",
          seq: 1,
          after_tool_seq: 1,
          kind: "queued_user",
          role: "user",
          content: "补充一句",
          created_at: "2026-08-14T10:00:05Z"
        }]
      })
    ]));

    expect(model.records.map((record) => record.kind)).toEqual([
      "user", "tool", "message", "tool", "assistant"
    ]);
  });

  it("失败轮把错误挂在助手记录上", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        status: "failed",
        assistant: { timestamp: "2026-08-14T10:00:10Z", content: "" },
        error: "provider timeout"
      })
    ]));

    const assistant = model.records.find((record) => record.kind === "assistant");
    expect(assistant?.failed).toBe(true);
    expect(assistant?.detail.error).toBe("provider timeout");
  });

  it("旧历史缺少 assistant_round 时按工具序号分批", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        tools: [tool({ id: "c1", seq: 1 }), tool({ id: "c2", seq: 2 })]
      })
    ]));

    expect(model.records.filter((record) => record.kind === "tool").map((record) => record.round))
      .toEqual([1, 2]);
  });

  it("没有时间线时返回空模型", () => {
    expect(buildTrajectory(undefined)).toEqual({ records: [], turns: [] });
  });
});
