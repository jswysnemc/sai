import { describe, expect, it } from "vitest";
import type { SessionTimelineTurn } from "../../api/contracts";
import type { LiveRunState } from "./run-event-reducer";
import { projectConversationDisplay, retryableTurnId } from "./conversation-display";

/**
 * 构造展示投影测试使用的会话轮次。
 *
 * @param id 轮次标识
 * @param content 用户消息正文
 * @param status 轮次状态
 * @returns 会话时间线轮次
 */
function turn(id: string, content: string, status: SessionTimelineTurn["status"]): SessionTimelineTurn {
  return {
    turn_id: id,
    seq: 1,
    status,
    automatic: false,
    user: { timestamp: "now", content },
    assistant: { timestamp: "later", content: status === "running" ? "" : "answer" },
    tools: []
  };
}

/**
 * 构造展示投影测试使用的实时运行。
 *
 * @param id 运行标识，同时也是 Web 轮次标识
 * @param content 用户消息正文
 * @param completed 运行是否结束
 * @returns 实时运行状态
 */
function run(id: string, content: string, completed: boolean): LiveRunState {
  return {
    runId: id,
    sessionId: "session",
    status: completed ? "idle" : "working",
    userInput: content,
    imageUrls: [],
    content: completed ? "answer" : "",
    reasoning: "",
    tools: [],
    parts: [],
    error: null,
    errorDetail: null,
    completed
  };
}

describe("projectConversationDisplay", () => {
  it("renders a running turn only from live state", () => {
    const projection = projectConversationDisplay(
      [turn("run-1", "inspect", "running")],
      [run("run-1", "inspect", false)]
    );

    expect(projection.historyTurns).toEqual([]);
    expect(projection.liveRuns.map((item) => item.runId)).toEqual(["run-1"]);
  });

  it("renders a completed turn only from durable history", () => {
    const durable = turn("run-1", "inspect", "interrupted");
    const projection = projectConversationDisplay([durable], [run("run-1", "inspect", true)]);

    expect(projection.historyTurns).toEqual([durable]);
    expect(projection.liveRuns).toEqual([]);
  });

  it("preserves two intentional turns with identical text", () => {
    const first = turn("run-1", "continue", "completed");
    const projection = projectConversationDisplay(
      [first],
      [run("run-1", "continue", true), run("run-2", "continue", false)]
    );

    expect(projection.historyTurns.map((item) => item.turn_id)).toEqual(["run-1"]);
    expect(projection.liveRuns.map((item) => item.runId)).toEqual(["run-2"]);
  });
});

describe("retryableTurnId", () => {
  it("returns the persisted turn matching the selected message", () => {
    expect(retryableTurnId([turn("run-1", "inspect", "interrupted")], "run-1"))
      .toBe("run-1");
  });

  it("does not roll back unrelated history for a preflight failure", () => {
    expect(retryableTurnId([turn("turn-old", "older", "completed")], "run-failed"))
      .toBeUndefined();
  });
});
