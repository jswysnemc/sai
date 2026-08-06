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

  it("updates queued input and moves the run to the requested queue position", () => {
    const first = sessionRunsReducer({ runs: [] }, {
      type: "start",
      run: {
        run_id: "run-1",
        workspace_id: "workspace",
        session_id: "session",
        status: "queued"
      },
      sessionId: "session",
      userInput: "first"
    });
    const second = sessionRunsReducer(first, {
      type: "start",
      run: {
        run_id: "run-2",
        workspace_id: "workspace",
        session_id: "session",
        status: "queued"
      },
      sessionId: "session",
      userInput: "second"
    });

    const updated = sessionRunsReducer(second, {
      type: "event",
      event: {
        sequence: 3,
        run_id: "run-2",
        workspace_id: "workspace",
        session_id: "session",
        timestamp: "now",
        type: "run.queue.updated",
        payload: { input: "edited", position: 0 }
      }
    });

    expect(updated.runs.map((run) => run.runId)).toEqual(["run-2", "run-1"]);
    expect(updated.runs[0].userInput).toBe("edited");
  });

  it("removes a queued run immediately after deletion succeeds", () => {
    const queued = sessionRunsReducer({ runs: [] }, {
      type: "start",
      run: {
        run_id: "run-q",
        workspace_id: "workspace",
        session_id: "session",
        status: "queued"
      },
      sessionId: "session",
      userInput: "delete me"
    });

    expect(sessionRunsReducer(queued, { type: "remove-queued", runId: "run-q" }).runs).toEqual([]);
  });

  it("removes a queued run after it is merged into the active model turn", () => {
    const queued = sessionRunsReducer({ runs: [] }, {
      type: "start",
      run: {
        run_id: "run-q",
        workspace_id: "workspace",
        session_id: "session",
        status: "queued"
      },
      sessionId: "session",
      userInput: "continue here"
    });

    const merged = sessionRunsReducer(queued, {
      type: "event",
      event: {
        sequence: 2,
        run_id: "run-q",
        workspace_id: "workspace",
        session_id: "session",
        timestamp: "now",
        type: "run.merged",
        payload: { target_run_id: "run-active" }
      }
    });

    expect(merged.runs).toEqual([]);
  });
});
