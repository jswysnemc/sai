import { describe, expect, it } from "vitest";
import type { WebEvent } from "../../api/contracts";
import { applyEventsToSessionRuns, sessionRunsReducer, upsertRunFromEvent } from "./use-run-stream";

/**
 * 构造服务端广播的事件。
 *
 * @param type 事件类型
 * @param payload 事件负载
 * @param runId 运行标识
 * @returns Web 运行事件
 */
function broadcast(type: string, payload: Record<string, unknown>, runId = "run-2"): WebEvent {
  return {
    sequence: 7,
    run_id: runId,
    workspace_id: "workspace",
    session_id: "session",
    timestamp: "now",
    type,
    payload
  };
}

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

  it("applies a batch of content deltas in one reducer pass", () => {
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
    const deltas: WebEvent[] = ["A", "B", "C"].map((chunk, index) => ({
      sequence: index + 1,
      run_id: "run-1",
      workspace_id: "workspace",
      session_id: "session",
      timestamp: "now",
      type: "message.content.delta",
      payload: { text: chunk }
    }));
    const batched = applyEventsToSessionRuns(started, deltas);
    expect(batched.runs).toHaveLength(1);
    expect(batched.runs[0].content).toContain("A");
    expect(batched.runs[0].content).toContain("B");
    expect(batched.runs[0].content).toContain("C");
  });

  it("prunes completed live runs once timeline turn ids are known", () => {
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
    const completed = sessionRunsReducer(started, {
      type: "event",
      event: {
        sequence: 2,
        run_id: "run-1",
        workspace_id: "workspace",
        session_id: "session",
        timestamp: "now",
        type: "run.completed",
        payload: {}
      }
    });
    expect(completed.runs).toHaveLength(1);
    expect(completed.runs[0].completed).toBe(true);

    const pruned = sessionRunsReducer(completed, {
      type: "prune-settled",
      historyTurnIds: ["run-1"]
    });
    expect(pruned.runs).toEqual([]);

    const kept = sessionRunsReducer(completed, {
      type: "prune-settled",
      historyTurnIds: ["other-turn"]
    });
    expect(kept.runs).toHaveLength(1);
  });
});

describe("session runs server-driven upsert", () => {
  it("builds the run entry from a broadcast run.started so a second tab sees the turn", () => {
    const started = sessionRunsReducer({ runs: [] }, {
      type: "event",
      event: broadcast("run.started", { input: "第二标签也能看到", image_urls: [] })
    });

    expect(started.runs).toHaveLength(1);
    expect(started.runs[0].runId).toBe("run-2");
    expect(started.runs[0].userInput).toBe("第二标签也能看到");
    expect(started.runs[0].status).toBe("waiting_response");
    expect(started.runs[0].completed).toBe(false);
  });

  it("keeps one user bubble when run.started arrives after the local dispatch", () => {
    const local = sessionRunsReducer({ runs: [] }, {
      type: "start",
      run: { run_id: "run-2", workspace_id: "workspace", session_id: "session", status: "running" },
      sessionId: "session",
      userInput: "界面显示正文"
    });

    const afterServer = sessionRunsReducer(local, {
      type: "event",
      event: broadcast("run.started", { input: "带旁路上下文的完整输入", image_urls: [] })
    });

    expect(afterServer.runs).toHaveLength(1);
    expect(afterServer.runs[0].userInput).toBe("界面显示正文");
    expect(afterServer.runs[0].status).toBe("waiting_response");
  });

  it("keeps one user bubble when the local dispatch arrives after the broadcast", () => {
    const fromServer = sessionRunsReducer({ runs: [] }, {
      type: "event",
      event: broadcast("run.started", { input: "带旁路上下文的完整输入", image_urls: [] })
    });

    const afterLocal = sessionRunsReducer(fromServer, {
      type: "start",
      run: { run_id: "run-2", workspace_id: "workspace", session_id: "session", status: "running" },
      sessionId: "session",
      userInput: "界面显示正文"
    });

    expect(afterLocal.runs).toHaveLength(1);
    expect(afterLocal.runs[0].userInput).toBe("界面显示正文");
  });

  it("marks a broadcast run as queued and carries its input", () => {
    const queued = sessionRunsReducer({ runs: [] }, {
      type: "event",
      event: broadcast("run.queued", { position: 1, input: "排队中的消息", image_urls: [] })
    });

    expect(queued.runs).toHaveLength(1);
    expect(queued.runs[0].status).toBe("queued");
    expect(queued.runs[0].userInput).toBe("排队中的消息");
  });

  it("creates the run once when the same entry event is replayed", () => {
    const event = broadcast("run.started", { input: "hello", image_urls: [] });
    const first = upsertRunFromEvent({ runs: [] }, event);
    const second = upsertRunFromEvent(first, event);

    expect(first.runs).toHaveLength(1);
    expect(second.runs).toHaveLength(1);
    expect(second.runs[0]).toEqual(first.runs[0]);
    expect(second.runs[0].startedAtMs).toBe(first.runs[0].startedAtMs);
  });

  it("replays a full turn into an empty tab state", () => {
    const events: WebEvent[] = [
      broadcast("run.started", { input: "问题", image_urls: [] }, "run-9"),
      broadcast("message.content.delta", { text: "回答" }, "run-9"),
      broadcast("run.completed", { duration_ms: 120 }, "run-9")
    ];

    const replayed = applyEventsToSessionRuns({ runs: [] }, events);

    expect(replayed.runs).toHaveLength(1);
    expect(replayed.runs[0].userInput).toBe("问题");
    expect(replayed.runs[0].content).toBe("回答");
    expect(replayed.runs[0].completed).toBe(true);
  });

  it("ignores content deltas for runs the tab never opened", () => {
    const applied = sessionRunsReducer({ runs: [] }, {
      type: "event",
      event: broadcast("message.content.delta", { text: "无关增量" }, "run-unknown")
    });

    expect(applied.runs).toEqual([]);
  });

  it("ignores stream.lagged because the client reconnects instead", () => {
    const applied = sessionRunsReducer({ runs: [] }, {
      type: "event",
      event: broadcast("stream.lagged", { dropped: 3 }, "")
    });

    expect(applied.runs).toEqual([]);
  });

  it("fails every open run when the session stream gives up", () => {
    const running = sessionRunsReducer({ runs: [] }, {
      type: "event",
      event: broadcast("run.started", { input: "进行中", image_urls: [] }, "run-open")
    });
    const settled = sessionRunsReducer(running, {
      type: "prune-settled",
      historyTurnIds: ["run-done"]
    });

    const failed = sessionRunsReducer(settled, { type: "fail-open", summary: "连接中断", detail: "detail" });

    expect(failed.runs).toHaveLength(1);
    expect(failed.runs[0].completed).toBe(true);
    expect(failed.runs[0].error).toBe("连接中断");
  });
});
