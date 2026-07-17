import { describe, expect, it } from "vitest";
import { resolveComposerAvailability } from "./composer-availability";

describe("composer availability", () => {
  it.each(["queued", "waiting_response", "thinking", "working"] as const)("运行阶段 %s 允许继续提交到会话队列", (runStatus) => {
    expect(resolveComposerAvailability({ sessionAvailable: true, runActive: true, runStatus, hasDraft: true })).toEqual({
      inputDisabled: false,
      sendDisabled: false,
      showStop: true
    });
  });

  it("空闲且存在草稿时允许发送", () => {
    expect(resolveComposerAvailability({ sessionAvailable: true, runActive: false, runStatus: "idle", hasDraft: true })).toEqual({
      inputDisabled: false,
      sendDisabled: false,
      showStop: false
    });
  });

  it("未选择会话时禁止编辑和发送", () => {
    expect(resolveComposerAvailability({ sessionAvailable: false, runActive: false, runStatus: "idle", hasDraft: true })).toEqual({
      inputDisabled: true,
      sendDisabled: true,
      showStop: false
    });
  });

  it("运行状态已到达但运行标记尚未同步时仍允许排队", () => {
    expect(resolveComposerAvailability({ sessionAvailable: true, runActive: false, runStatus: "thinking", hasDraft: true }).sendDisabled).toBe(false);
  });
});
