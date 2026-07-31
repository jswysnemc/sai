import { describe, expect, it } from "vitest";
import { isConversationEmpty, shouldCenterEmptySession } from "./empty-session-layout";

const EMPTY_SNAPSHOT = {
  timelineLoading: false,
  historyTurnCount: 0,
  liveRunCount: 0,
  hasHistoryCompaction: false
};

describe("empty session layout", () => {
  it("仅在时间线完成读取且没有任何内容时认定为空会话", () => {
    expect(isConversationEmpty(EMPTY_SNAPSHOT)).toBe(true);
    expect(isConversationEmpty({ ...EMPTY_SNAPSHOT, timelineLoading: true })).toBe(false);
    expect(isConversationEmpty({ ...EMPTY_SNAPSHOT, historyTurnCount: 1 })).toBe(false);
    expect(isConversationEmpty({ ...EMPTY_SNAPSHOT, liveRunCount: 1 })).toBe(false);
    expect(isConversationEmpty({ ...EMPTY_SNAPSHOT, hasHistoryCompaction: true })).toBe(false);
  });

  it("提交前居中，提交后立即切换到底部布局", () => {
    expect(shouldCenterEmptySession(true, "session-a", null)).toBe(true);
    expect(shouldCenterEmptySession(true, "session-a", "session-a")).toBe(false);
  });

  it("前一会话的提交状态不影响新切换的空会话", () => {
    expect(shouldCenterEmptySession(true, "session-b", "session-a")).toBe(true);
  });

  it("没有活动会话或已有会话内容时不展示居中空态", () => {
    expect(shouldCenterEmptySession(true, undefined, null)).toBe(false);
    expect(shouldCenterEmptySession(false, "session-a", null)).toBe(false);
  });
});
