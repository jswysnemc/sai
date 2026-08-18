import { describe, expect, it } from "vitest";
import type { SessionDebugRequest, SessionTimeline, SessionTimelineTurn, TimelineToolEntry } from "../../api/contracts";
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

describe("buildTrajectory 的系统提示词记录", () => {
  /** 构造一份系统提示词快照。 */
  const prompt = {
    source: "session_baseline",
    content: "You are Sai.",
    char_count: 12,
    token_count: 7140,
    has_instruction_files: true,
    has_skills: true,
    has_tools: true,
    has_memory: false,
    has_dynamic: false,
    tool_count: 17,
    sections: [
      { id: "baseline", label: "会话 baseline", content: "You are Sai." },
      { id: "tools", label: "工具定义", content: "read_file..." }
    ]
  };

  it("排在全部轮次之前", () => {
    const model = buildTrajectory(
      timeline([turn({ turn_id: "t1" })]),
      prompt as never
    );

    expect(model.records[0].kind).toBe("system");
    expect(model.records[0].index).toBe(1);
    expect(model.records[1].kind).toBe("user");
  });

  it("摘要给出体量与构成而不是正文开头", () => {
    const model = buildTrajectory(undefined, prompt as never);

    expect(model.records[0].summary).toBe("7140 tokens · 17 tools · 会话 baseline · 工具定义");
  });

  it("分区进入详情供分段展示", () => {
    const model = buildTrajectory(undefined, prompt as never);

    expect(model.records[0].detail.sections).toHaveLength(2);
    expect(model.records[0].detail.input).toBe("You are Sai.");
  });

  it("没有系统提示词时不插入空记录", () => {
    expect(buildTrajectory(timeline([turn({ turn_id: "t1" })])).records[0].kind).toBe("user");
  });

  it("正文为空白的快照同样跳过", () => {
    const model = buildTrajectory(undefined, { ...prompt, content: "   " } as never);

    expect(model.records).toHaveLength(0);
  });

  it("优先展示真实请求体中的系统提示词和工具定义", () => {
    const request: SessionDebugRequest = {
      request_id: "req-1",
      turn_id: "t1",
      assistant_round: 1,
      request_body: {
        messages: [{ role: "system", content: "Actual system prompt" }],
        tools: [{ type: "function", function: { name: "read_file" } }]
      }
    };
    const model = buildTrajectory(timeline([turn({ turn_id: "t1" })]), prompt as never, undefined, [request]);
    const actual = model.records.find((record) => record.detail.actualRequest);

    expect(model.records[0].kind).toBe("system");
    expect(model.records[0].label).toBe("baseline");
    expect(actual?.label).toBe("actual #1");
    expect(actual?.detail.sections?.map((section) => section.label)).toEqual([
      "System prompt",
      "Tool definitions"
    ]);
    expect(actual?.detail.sections?.[0].content).toBe("Actual system prompt");
  });

  it("从 instructions 与 developer 角色补全系统提示", () => {
    const request: SessionDebugRequest = {
      request_id: "req-2",
      turn_id: "t1",
      assistant_round: 1,
      request_body: {
        instructions: "Responses instructions",
        messages: [{ role: "developer", content: "Developer brief" }]
      }
    };
    const model = buildTrajectory(timeline([turn({ turn_id: "t1" })]), undefined, undefined, [request]);
    const actual = model.records.find((record) => record.detail.actualRequest);

    expect(actual?.detail.sections?.[0].content).toBe("Responses instructions\n\nDeveloper brief");
  });
});

describe("buildTrajectory 的思考与注入", () => {
  it("把工具批次上的思考抽成独立记录", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        tools: [tool({
          id: "c1",
          seq: 1,
          assistant_round: 1,
          reasoning: "先读配置"
        })]
      })
    ]));

    expect(model.records.map((record) => record.kind)).toEqual(["user", "thinking", "tool", "assistant"]);
    expect(model.records.find((record) => record.kind === "thinking")?.detail.reasoning).toBe("先读配置");
  });

  it("助手只剩思考时不再挂在模型行上", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        assistant: { timestamp: "2026-08-14T10:00:10Z", content: "", reasoning: "想清楚再答" }
      })
    ]));

    expect(model.records.map((record) => record.kind)).toEqual(["user", "thinking"]);
    expect(model.records[1].detail.reasoning).toBe("想清楚再答");
  });

  it("把压缩摘要插到被覆盖轮次之后、保留轮次之前", () => {
    const model = buildTrajectory({
      turns: [
        turn({ turn_id: "t1", seq: 1 }),
        turn({ turn_id: "t2", seq: 2 }),
        turn({ turn_id: "t3", seq: 3 }),
        turn({ turn_id: "t4", seq: 4 }),
        turn({ turn_id: "t5", seq: 5, user: { timestamp: "2026-08-14T10:20:00Z", content: "继续排查" } })
      ],
      compaction: {
        applied: true,
        turn_count: 4,
        compacted_from_seq: 1,
        compacted_to_seq: 4,
        summary: "## 1. Primary Request\n排查崩溃",
        created_at: "2026-08-14T10:15:00Z",
        reason: "auto"
      }
    });

    expect(model.turns.map((item) => item.seq)).toEqual([1, 2, 3, 4, 5]);
    const kinds = model.records.map((record) => `${record.kind}:${record.turnSeq ?? record.label ?? ""}`);
    const compactionAt = kinds.findIndex((item) => item.startsWith("compaction:"));
    const turn5At = kinds.findIndex((item) => item === "user:5");
    expect(compactionAt).toBeGreaterThan(-1);
    expect(turn5At).toBeGreaterThan(compactionAt);
    expect(kinds[compactionAt - 1]).toBe("assistant:4");
  });

  it("没有后续轮次时压缩摘要仍落在末尾", () => {
    const model = buildTrajectory({
      turns: [turn({ turn_id: "t1", seq: 1 })],
      compaction: {
        applied: true,
        turn_count: 1,
        summary: "旧轮次摘要",
        created_at: "2026-08-14T10:15:00Z",
        reason: "auto"
      }
    });

    expect(model.records.at(-1)?.kind).toBe("compaction");
  });

  it("被删掉的旧轮次把压缩摘要插到剩余对话前面", () => {
    const model = buildTrajectory({
      turns: [
        turn({ turn_id: "t4", seq: 4, user: { timestamp: "2026-08-14T10:20:00Z", content: "当前可用哪些ssh主机" } }),
        turn({ turn_id: "t5", seq: 5, user: { timestamp: "2026-08-14T10:21:00Z", content: "连上local试试" } })
      ],
      compaction: {
        applied: true,
        turn_count: 3,
        compacted_from_seq: 1,
        compacted_to_seq: 3,
        summary: "前三轮已折进摘要",
        created_at: "2026-08-14T10:15:00Z",
        reason: "auto"
      }
    }, {
      source: "session_baseline",
      content: "You are Sai.",
      char_count: 12,
      token_count: 10768,
      has_instruction_files: false,
      has_skills: false,
      has_tools: true,
      has_memory: false,
      has_dynamic: false,
      tool_count: 17,
      sections: []
    } as never);

    expect(model.turns.map((item) => item.seq)).toEqual([4, 5]);
    expect(model.records.map((record) => record.kind)).toEqual([
      "system",
      "compaction",
      "user",
      "assistant",
      "user",
      "assistant"
    ]);
    expect(model.records[1].detail.compactedFromSeq).toBe(1);
    expect(model.records[1].detail.compactedToSeq).toBe(3);
  });

  it("把供应商用户消息里的注入前缀展成插入记录", () => {
    const model = buildTrajectory(timeline([
      turn({
        turn_id: "t1",
        injected_content: "<context-state>\n{\"kind\":\"runtime_change\"}\n</context-state>"
      })
    ]));

    const injected = model.records.find((record) => record.kind === "message");
    expect(injected?.label).toBe("context-state");
    expect(model.records.map((record) => record.kind)).toEqual(["user", "message", "assistant"]);
  });
});
