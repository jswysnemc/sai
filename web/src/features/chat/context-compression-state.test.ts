import { describe, expect, it } from "vitest";
import { initialRunState, runEventReducer, type LiveRunState } from "./run-event-reducer";
import { groupActivityParts } from "./message/group-activity-parts";

/**
 * 【上下文】【事件回归】通过正式归并器处理一条工具事件。
 * @param state 前一状态
 * @param type 事件类型
 * @param payload 事件载荷
 * @returns 新运行状态
 */
function send(state: LiveRunState, type: string, payload: Record<string, unknown>): LiveRunState {
  return runEventReducer(state, { type: "event", event: {
    sequence: 1, run_id: "run", workspace_id: "workspace", session_id: "session",
    timestamp: "2026-10-03T12:00:00Z", type, payload
  } });
}

/**
 * 【上下文】【事件回归】创建正在生成摘要的运行状态。
 * @returns 压缩参数流状态
 */
function preparing(): LiveRunState {
  const state = runEventReducer(initialRunState, { type: "start", runId: "run", sessionId: "session", userInput: "检查源码" });
  return send(state, "tool.call.preparing", { tool_id: "compress-1", name: "compress_context", arguments_preview: '{"summary":"' });
}

describe("上下文压缩反馈", () => {
  it("参数流开始显示压缩状态，完成后保留独立卡片并继续等待模型", () => {
    let state = preparing();
    expect(state.status).toBe("compacting");
    state = send(state, "status.changed", { status: "working" });
    expect(state.status).toBe("compacting");
    state = send(state, "tool.call.started", { tool_id: "compress-1", name: "compress_context", arguments: '{"topic":"检查"}' });
    state = send(state, "tool.result", { tool_id: "compress-1", name: "compress_context", ok: true, output: '{"before_tokens":12000,"after_tokens":800}' });
    expect(state.status).toBe("waiting_response");
    expect(state.tools).toHaveLength(1);
    expect(state.tools[0].status).toBe("completed");
    expect(groupActivityParts(state.parts)[0].type).toBe("part");
  });

  it("并行工具事件不会提前结束压缩提示", () => {
    let state = send(preparing(), "tool.call.started", { tool_id: "read-1", name: "read_file", arguments: "{}" });
    expect(state.status).toBe("compacting");
    state = send(state, "tool.result", { tool_id: "compress-1", name: "compress_context", ok: false, output: "stale context revision" });
    expect(state.status).toBe("working");
    expect(state.error).toBeNull();
    expect(state.tools[0].status).toBe("failed");
  });

  it.each(["run.failed", "run.interrupted", "run.completed"])("%s 收敛未结束的压缩卡片", (type) => {
    const state = send(preparing(), type, { message: "connection closed" });
    expect(state.completed).toBe(true);
    expect(state.status).toBe("idle");
    const part = state.parts.find((part) => part.type === "tool");
    expect(part?.type === "tool" && part.tool.status).toBe("failed");
  });
});
