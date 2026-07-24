import { describe, expect, it } from "vitest";
import type { WebEvent } from "../../api/contracts";
import { sessionRunsReducer } from "./use-run-stream";

/**
 * 构造会话运行测试事件。
 *
 * @param payload 中断事件负载
 * @returns Web 运行事件
 */
function event(payload: Record<string, unknown>): WebEvent {
  return {
    sequence: 1,
    run_id: "run-1",
    workspace_id: "workspace",
    session_id: "session",
    timestamp: "now",
    type: "run.interrupted",
    payload
  };
}

describe("sessionRunsReducer", () => {
  it("removes the live user bubble when interruption has no assistant reply", () => {
    const started = sessionRunsReducer({ runs: [] }, {
      type: "start",
      run: {
        run_id: "run-1",
        workspace_id: "workspace",
        session_id: "session",
        input: "edit me",
        image_urls: [],
        status: "running"
      },
      sessionId: "session",
      userInput: "edit me"
    });

    const interrupted = sessionRunsReducer(started, {
      type: "event",
      event: event({ discard_user_turn: true, restore_input: "edit me" })
    });

    expect(interrupted.runs).toEqual([]);
  });

  it("marks a running turn completed when stop is applied locally", () => {
    const started = sessionRunsReducer({ runs: [] }, {
      type: "start",
      run: {
        run_id: "run-1",
        workspace_id: "workspace",
        session_id: "session",
        input: "hello",
        image_urls: [],
        status: "running"
      },
      sessionId: "session",
      userInput: "hello"
    });
    const thinking = sessionRunsReducer(started, {
      type: "event",
      event: {
        sequence: 2,
        run_id: "run-1",
        workspace_id: "workspace",
        session_id: "session",
        timestamp: "now",
        type: "status.changed",
        payload: { status: "thinking" }
      }
    });
    const stopped = sessionRunsReducer(thinking, { type: "stop-local", runId: "run-1" });
    expect(stopped.runs).toHaveLength(1);
    expect(stopped.runs[0].completed).toBe(true);
    expect(stopped.runs[0].status).toBe("idle");
  });
});
