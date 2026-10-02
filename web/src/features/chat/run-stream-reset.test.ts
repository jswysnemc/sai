import { describe, expect, it } from "vitest";
import { parseRunStreamReset } from "./run-stream-reset";
import { sessionRunsReducer } from "./session-runs-reducer";
import type { WebEvent } from "../../api/contracts";

/** 【会话同步】【测试事件】参数为类型、序号和载荷，返回固定会话事件。 */
function event(type: string, sequence: number, payload: Record<string, unknown>): WebEvent {
  return { type, sequence, payload, run_id: "run", workspace_id: "workspace", session_id: "session", timestamp: "now" };
}

describe("stream.reset", () => {
  it("整体替换半截正文，恢复权限并只接受水位之后的增量", () => {
    const entry = event("run.started", 1, { input: "question" });
    const partial = sessionRunsReducer({ runs: [] }, { type: "events", events: [entry, event("message.content.delta", 2, { text: "old" })] });
    const reset = parseRunStreamReset(JSON.stringify({ through_sequence: 4000, incomplete_run_ids: [], events: [
      entry, event("message.content.delta", 3999, { text: "complete" }),
      event("permission.requested", 4000, { request_id: "permission", tool: "bash", arguments: {} })
    ] }), "workspace", "session")!;
    expect(reset.events.every((value) => value.replayed)).toBe(true);
    const restored = sessionRunsReducer(partial, { type: "restore", events: reset.events });
    expect(restored.runs[0].content).toBe("complete");
    expect(restored.runs[0].replayed).toBe(true);
    const next = sessionRunsReducer(restored, { type: "event", event: event("message.content.delta", 4001, { text: "!" }) });
    expect(next.runs[0].content).toBe("complete!");
    expect(next.runs[0].replayed).toBe(false);
  });

  it("拒绝跨工作区、跨会话、乱序和无效水位", () => {
    const entry = event("run.started", 1, {});
    const encoded = JSON.stringify({ through_sequence: 1, events: [entry], incomplete_run_ids: [] });
    expect(parseRunStreamReset(encoded, "other", "session")).toBeNull();
    expect(parseRunStreamReset(encoded, "workspace", "other")).toBeNull();
    expect(parseRunStreamReset('{"through_sequence":-1,"events":[],"incomplete_run_ids":[]}', "workspace", "session")).toBeNull();
    expect(parseRunStreamReset(JSON.stringify({ through_sequence: 1, events: [entry, entry], incomplete_run_ids: [] }), "workspace", "session")).toBeNull();
  });

  it("允许服务端序号回退并清理旧运行", () => {
    const state = sessionRunsReducer({ runs: [] }, { type: "event", event: event("run.started", 50, {}) });
    const reset = parseRunStreamReset('{"through_sequence":0,"events":[],"incomplete_run_ids":[]}', "workspace", "session")!;
    expect(sessionRunsReducer(state, { type: "restore", events: reset.events }).runs).toEqual([]);
  });
});
