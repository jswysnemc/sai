import { describe, expect, it } from "vitest";
import type { WebEvent } from "../../api/contracts";
import { initialRunState, relocalizeRunError, runEventReducer } from "./run-event-reducer";

function event(type: string, payload: Record<string, unknown>): WebEvent {
  return { sequence: 1, run_id: "run", workspace_id: "workspace", session_id: "session", timestamp: "now", type, payload };
}

describe("runEventReducer", () => {
  it("streams reasoning and content independently", () => {
    const started = runEventReducer(initialRunState, { type: "start", runId: "run", sessionId: "session", userInput: "hello" });
    const reasoning = runEventReducer(started, { type: "event", event: event("message.reasoning.delta", { text: "think" }) });
    const content = runEventReducer(reasoning, { type: "event", event: event("message.content.delta", { text: "answer" }) });
    expect(content.reasoning).toBe("think");
    expect(content.content).toBe("answer");
    expect(content.parts.map((part) => part.type)).toEqual(["reasoning", "text"]);
  });

  it("updates one tool card through its lifecycle", () => {
    const preparing = runEventReducer(initialRunState, { type: "event", event: event("tool.call.preparing", { tool_id: "tool", name: "edit_file", arguments_preview: "partial" }) });
    const running = runEventReducer(preparing, { type: "event", event: event("tool.call.started", { tool_id: "tool", name: "edit_file", arguments: "{}" }) });
    const completed = runEventReducer(running, { type: "event", event: event("tool.result", { tool_id: "tool", name: "edit_file", ok: true, output: "ok" }) });
    expect(completed.tools).toHaveLength(1);
    expect(completed.tools[0].status).toBe("completed");
    expect(completed.tools[0].output).toBe("ok");
    expect(completed.parts).toHaveLength(1);
    expect(completed.parts[0].type).toBe("tool");
  });

  it("keeps a tool at its original position when later content arrives", () => {
    const first = runEventReducer(initialRunState, { type: "event", event: event("message.content.delta", { text: "before" }) });
    const tool = runEventReducer(first, { type: "event", event: event("tool.call.started", { tool_id: "tool", name: "run_command", arguments: "{}" }) });
    const after = runEventReducer(tool, { type: "event", event: event("message.content.delta", { text: "after" }) });
    const completed = runEventReducer(after, { type: "event", event: event("tool.result", { tool_id: "tool", name: "run_command", ok: true, output: "ok" }) });
    expect(completed.parts.map((part) => part.type)).toEqual(["text", "tool", "text"]);
  });

  it("shows compaction progress in the live message timeline", () => {
    const started = runEventReducer(initialRunState, { type: "event", event: event("compaction.started", { turn_count: 8 }) });
    const finished = runEventReducer(started, {
      type: "event",
      event: event("compaction.finished", {
        applied: true,
        summary: "## Goal\n- keep context short"
      })
    });

    expect(finished.parts).toEqual([
      expect.objectContaining({
        type: "compaction",
        status: "completed",
        turnCount: 8,
        applied: true,
        summary: "## Goal\n- keep context short"
      })
    ]);
  });

  it("omits summary when compaction is not applied", () => {
    const started = runEventReducer(initialRunState, { type: "event", event: event("compaction.started", { turn_count: 3 }) });
    const finished = runEventReducer(started, {
      type: "event",
      event: event("compaction.finished", { applied: false, summary: "should not show" })
    });

    expect(finished.parts).toEqual([
      expect.objectContaining({ type: "compaction", status: "completed", turnCount: 3, applied: false, summary: undefined })
    ]);
  });

  it("streams compaction summary into the active compaction part", () => {
    const started = runEventReducer(initialRunState, { type: "event", event: event("compaction.started", { turn_count: 4 }) });
    const first = runEventReducer(started, { type: "event", event: event("compaction.delta", { text: "## 目标\n" }) });
    const second = runEventReducer(first, { type: "event", event: event("compaction.delta", { text: "保留上下文" }) });

    expect(second.parts).toEqual([
      expect.objectContaining({
        type: "compaction",
        status: "running",
        summary: "## 目标\n保留上下文"
      })
    ]);
  });

  it("keeps expandable compaction error details", () => {
    const started = runEventReducer(initialRunState, { type: "event", event: event("compaction.started", { turn_count: 4 }) });
    const finished = runEventReducer(started, {
      type: "event",
      event: event("compaction.finished", {
        applied: false,
        error: { message: "上下文压缩失败", detail: "provider returned 502" }
      })
    });

    expect(finished.parts).toEqual([
      expect.objectContaining({
        type: "compaction",
        status: "completed",
        applied: false,
        error: { message: "上下文压缩失败", detail: "provider returned 502" }
      })
    ]);
  });

  it("keeps a write tool paused until the user handles its permission request", () => {
    const request = {
      id: "permission",
      session_id: "session",
      tool: "edit_file",
      arguments: "{\"path\":\"src/main.rs\"}"
    };
    const waiting = runEventReducer(initialRunState, {
      type: "event",
      event: event("permission.requested", request)
    });

    expect(waiting.status).toBe("waiting_permission");
    expect(waiting.parts).toEqual([
      expect.objectContaining({ type: "permission", request })
    ]);
  });

  it("renders automatic input as a separate message part", () => {
    const next = runEventReducer(initialRunState, {
      type: "event",
      event: event("message.automatic.input", {
        kind: "external_completion",
        content: "后台任务已完成"
      })
    });

    expect(next.status).toBe("waiting_response");
    expect(next.parts).toEqual([
      expect.objectContaining({ type: "automatic_input", source: "后台任务已完成" })
    ]);
  });

  it("does not duplicate a permission card after an SSE replay", () => {
    const request = {
      id: "permission",
      session_id: "session",
      tool: "run_command",
      arguments: "{\"command\":\"cargo test\"}"
    };
    const first = runEventReducer(initialRunState, {
      type: "event",
      event: event("permission.requested", request)
    });
    const replayed = runEventReducer(first, {
      type: "event",
      event: event("permission.requested", request)
    });

    expect(replayed.parts.filter((part) => part.type === "permission")).toHaveLength(1);
  });

  it("keeps a resolved permission decision after an SSE replay", () => {
    const request = {
      id: "permission",
      session_id: "session",
      tool: "edit_file",
      arguments: "{\"path\":\"src/main.rs\"}"
    };
    const waiting = runEventReducer(initialRunState, {
      type: "event",
      event: event("permission.requested", request)
    });
    const resolved = runEventReducer(waiting, {
      type: "event",
      event: event("permission.resolved", {
        request_id: request.id,
        decision: { decision: "deny", reply: "保留该文件" }
      })
    });

    expect(resolved.parts).toEqual([
      expect.objectContaining({
        type: "permission",
        decision: { decision: "deny", reply: "保留该文件" }
      })
    ]);
  });

  it("keeps partial assistant content when a run is interrupted", () => {
    const content = runEventReducer(initialRunState, {
      type: "event",
      event: event("message.content.delta", { text: "partial" })
    });
    const interrupted = runEventReducer(content, {
      type: "event",
      event: event("run.interrupted", {})
    });

    expect(interrupted.content).toBe("partial");
    expect(interrupted.completed).toBe(true);
    expect(interrupted.error).toContain("已保留");
  });

  it("relocalizes built-in run errors after the interface language changes", () => {
    expect(relocalizeRunError("运行已中断", "en-US")).toBe("The run was interrupted");
    expect(relocalizeRunError("Run failed", "zh-CN")).toBe("运行失败");
    expect(relocalizeRunError("provider error", "zh-CN")).toBe("provider error");
  });
});
