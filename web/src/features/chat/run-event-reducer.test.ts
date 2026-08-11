import { describe, expect, it } from "vitest";
import type { WebEvent } from "../../api/contracts";
import { initialRunState, relocalizeRunError, runEventReducer } from "./run-event-reducer";

function event(type: string, payload: Record<string, unknown>): WebEvent {
  return { sequence: 1, run_id: "run", workspace_id: "workspace", session_id: "session", timestamp: "now", type, payload };
}

describe("runEventReducer", () => {
  it("upgrades thinking status when content arrives without status.changed", () => {
    const started = runEventReducer(initialRunState, { type: "start", runId: "run", sessionId: "session", userInput: "hello" });
    const thinking = runEventReducer(started, { type: "event", event: event("status.changed", { status: "thinking" }) });
    const content = runEventReducer(thinking, { type: "event", event: event("message.content.delta", { text: "answer" }) });
    expect(content.status).toBe("working");
    expect(content.content).toBe("answer");
  });

  it("tracks reconnecting attempts and clears them after recovery", () => {
    const started = runEventReducer(initialRunState, { type: "start", runId: "run", sessionId: "session", userInput: "hello" });
    const reconnecting = runEventReducer(started, {
      type: "event",
      event: event("status.changed", { status: "reconnecting", attempt: 2, max_attempts: 3 })
    });
    expect(reconnecting.status).toBe("reconnecting");
    expect(reconnecting.reconnectAttempt).toBe(2);
    expect(reconnecting.reconnectMaxAttempts).toBe(3);

    const recovered = runEventReducer(reconnecting, {
      type: "event",
      event: event("status.changed", { status: "waiting_response" })
    });
    expect(recovered.status).toBe("waiting_response");
    expect(recovered.reconnectAttempt).toBeNull();
    expect(recovered.reconnectMaxAttempts).toBeNull();
  });

    it("ignores thinking status after content started", () => {
    const started = runEventReducer(initialRunState, { type: "start", runId: "run", sessionId: "session", userInput: "hello" });
    const content = runEventReducer(started, { type: "event", event: event("message.content.delta", { text: "answer" }) });
    const thinking = runEventReducer(content, { type: "event", event: event("status.changed", { status: "thinking" }) });
    expect(thinking.status).toBe("working");
  });

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

  it("replaces the progressive gateway name without creating a second tool card", () => {
    const preparing = runEventReducer(initialRunState, {
      type: "event",
      event: event("tool.call.preparing", {
        tool_id: "call-1",
        name: "invoke_tool",
        arguments_preview: "{\"tool_name\":\"read_file\""
      })
    });
    const running = runEventReducer(preparing, {
      type: "event",
      event: event("tool.call.started", {
        tool_id: "call-1",
        name: "read_file",
        arguments: "{\"path\":\"README.md\"}"
      })
    });

    expect(running.tools).toHaveLength(1);
    expect(running.parts).toHaveLength(1);
    expect(running.tools[0]).toMatchObject({
      id: "call-1",
      name: "read_file",
      arguments: "{\"path\":\"README.md\"}",
      status: "running"
    });
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

  it("merges a granted permission into the tool card it authorized", () => {
    const request = {
      id: "permission",
      session_id: "session",
      tool: "run_command",
      arguments: "{\"command\":\"rm -rf tmp\"}"
    };
    const waiting = runEventReducer(initialRunState, {
      type: "event",
      event: event("permission.requested", request)
    });
    const granted = runEventReducer(waiting, {
      type: "event",
      event: event("permission.resolved", {
        request_id: request.id,
        decision: { decision: "allow", source: "auto_audit", reason: "只读查询" }
      })
    });
    const called = runEventReducer(granted, {
      type: "event",
      event: event("tool.call.started", { tool_id: "call-1", name: "run_command", arguments: request.arguments })
    });

    // 同一次操作只留一张卡：独立权限卡消失，决定挂到工具上
    expect(called.parts.filter((part) => part.type === "permission")).toHaveLength(0);
    const toolPart = called.parts.find((part) => part.type === "tool");
    expect(toolPart).toMatchObject({
      type: "tool",
      tool: {
        name: "run_command",
        permission: { decision: "allow", source: "auto_audit", reason: "只读查询" }
      }
    });
  });

  it("keeps a denied permission as its own card since no tool runs", () => {
    const request = {
      id: "permission",
      session_id: "session",
      tool: "run_command",
      arguments: "{\"command\":\"rm -rf /\"}"
    };
    const waiting = runEventReducer(initialRunState, {
      type: "event",
      event: event("permission.requested", request)
    });
    const denied = runEventReducer(waiting, {
      type: "event",
      event: event("permission.resolved", {
        request_id: request.id,
        decision: { decision: "deny", reply: "风险过高" }
      })
    });

    expect(denied.parts.filter((part) => part.type === "permission")).toHaveLength(1);
  });

  it("does not attach a granted permission to an unrelated tool", () => {
    const request = {
      id: "permission",
      session_id: "session",
      tool: "run_command",
      arguments: "{\"command\":\"ls\"}"
    };
    const waiting = runEventReducer(initialRunState, {
      type: "event",
      event: event("permission.requested", request)
    });
    const granted = runEventReducer(waiting, {
      type: "event",
      event: event("permission.resolved", {
        request_id: request.id,
        decision: { decision: "allow", source: "human" }
      })
    });
    const other = runEventReducer(granted, {
      type: "event",
      event: event("tool.call.started", { tool_id: "call-1", name: "read_file", arguments: "{}" })
    });

    // 工具名不符时宁可保留独立权限卡，也不做错误关联
    expect(other.parts.filter((part) => part.type === "permission")).toHaveLength(1);
    const toolPart = other.parts.find((part) => part.type === "tool");
    expect(toolPart?.type).toBe("tool");
    expect(toolPart?.type === "tool" ? toolPart.tool.permission : "missing").toBeUndefined();
  });

  /// 外部内核连上后要留下可见证据，否则用户无法分辨本轮由谁执行。
  it("records which engine took over the turn", () => {
    const state = runEventReducer(initialRunState, {
      type: "event",
      event: event("engine.ready", { engine: "Codex", version: "1.1.7" })
    });

    expect(state.parts).toEqual([
      expect.objectContaining({ type: "engine_ready", engine: "Codex", version: "1.1.7" })
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

  it("does not attribute an interruption to the user when the event has no cause", () => {
    const interrupted = runEventReducer(initialRunState, {
      type: "event",
      event: event("run.interrupted", {})
    });

    expect(interrupted.error).toBe("运行已中断");
    expect(interrupted.errorDetail).toContain("未收到可确认的中断原因");
    expect(interrupted.errorDetail).not.toContain("用户在运行完成前主动停止了本轮");
  });

  it("keeps interruption details for the expandable error view", () => {
    const interrupted = runEventReducer(initialRunState, {
      type: "event",
      event: event("run.interrupted", {
        detail: "The upstream request exceeded its 120 second timeout."
      })
    });

    expect(interrupted.error).toBe("运行已中断");
    expect(interrupted.errorDetail).toBe("The upstream request exceeded its 120 second timeout.");
  });

  it("uses the failure message as details when no separate trace is available", () => {
    const failed = runEventReducer(initialRunState, {
      type: "event",
      event: event("run.failed", { message: "upstream timeout" })
    });

    expect(failed.error).toBe("upstream timeout");
    expect(failed.errorDetail).toBe("upstream timeout");
  });

  it("keeps completed turn usage for the message metrics", () => {
    const completed = runEventReducer(initialRunState, {
      type: "event",
      event: event("run.completed", {
        duration_ms: 5_000,
        usage: {
          prompt_tokens: 1_000,
          completion_tokens: 200,
          total_tokens: 1_200,
          cache_read_tokens: 900,
          cache_write_tokens: 10
        }
      })
    });

    expect(completed.durationMs).toBe(5_000);
    expect(completed.usage).toEqual({
      prompt_tokens: 1_000,
      completion_tokens: 200,
      total_tokens: 1_200,
      cache_read_tokens: 900,
      cache_write_tokens: 10
    });
  });

  it("relocalizes built-in run errors after the interface language changes", () => {
    expect(relocalizeRunError("运行已中断", "en-US")).toBe("The run was interrupted");
    expect(relocalizeRunError("Run failed", "zh-CN")).toBe("运行失败");
    expect(relocalizeRunError("provider error", "zh-CN")).toBe("provider error");
  });
});
