import { describe, expect, it } from "vitest";
import type { WebEvent } from "../../api/contracts";
import { planModeFromEvent } from "./plan-mode-event";

/** 【计划模式】【测试事件】参数为工具名、结果与是否回放，返回会话事件。 */
function event(name: string, result: object, replayed = false): WebEvent {
  return { sequence: 1, run_id: "run", workspace_id: "workspace", session_id: "session", timestamp: "2026-10-02T00:00:00Z", type: "tool.result", payload: { name, ok: true, output: JSON.stringify(result) }, replayed };
}

describe("plan mode events", () => {
  it("switches only after a successful explicit transition", () => {
    expect(planModeFromEvent(event("enter_plan_mode", { mode: "plan" }))).toBe("plan");
    expect(planModeFromEvent(event("exit_plan_mode", { approved: true, mode: "audited" }))).toBe("audited");
    expect(planModeFromEvent(event("exit_plan_mode", { approved: false, mode: "yolo" }))).toBeUndefined();
    expect(planModeFromEvent(event("exit_plan_mode", { approved: true, mode: "yolo" }, true))).toBeUndefined();
    expect(planModeFromEvent(event("run_command", { approved: true, mode: "yolo" }))).toBeUndefined();
  });
});
