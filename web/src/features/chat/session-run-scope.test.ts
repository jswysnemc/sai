import { describe, expect, it } from "vitest";
import type { WebEvent } from "../../api/contracts";
import { createSessionRunScope } from "./session-run-scope";

const event: WebEvent = {
  type: "message.content.delta", workspace_id: "workspace", session_id: "A",
  run_id: "run", sequence: 2, timestamp: "now", payload: { delta: "回答" }
};

describe("会话异步归属", () => {
  it("离开后重新进入同一会话也拒绝最初请求的迟到结果", () => {
    const scopes = createSessionRunScope();
    const old = scopes.select("workspace", "A");
    scopes.select("workspace", "B");
    const current = scopes.select("workspace", "A");
    expect(scopes.isCurrent(old)).toBe(false);
    expect(scopes.isCurrent(current)).toBe(true);
  });
  it("拒绝旧会话、其他工作区和重复投递的事件", () => {
    const scopes = createSessionRunScope();
    const current = scopes.select("workspace", "A");
    expect(scopes.accepts(current, event, 1)).toBe(true);
    expect(scopes.accepts(current, event, 2)).toBe(false);
    expect(scopes.accepts(current, { ...event, session_id: "B" }, 1)).toBe(false);
    expect(scopes.accepts(current, { ...event, workspace_id: "other" }, 1)).toBe(false);
    scopes.select("workspace", "B");
    expect(scopes.accepts(current, event, 1)).toBe(false);
  });
});
