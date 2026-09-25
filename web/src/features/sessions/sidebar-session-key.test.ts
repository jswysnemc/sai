import { describe, expect, it } from "vitest";
import {
  countSessionIds,
  matchesStoredSessionId,
  nextPinnedIds,
  sidebarSessionKey
} from "./sidebar-session-key";

describe("sidebar session key", () => {
  it("不同工作区的 default 会话不会共用一个键", () => {
    const left = sidebarSessionKey("workspace-a", "default");
    const right = sidebarSessionKey("workspace-b", "default");
    expect(left).not.toBe(right);
    expect(left).not.toBe("default");
  });

  it("重复的旧裸 ID 不再同时点亮两条会话", () => {
    const counts = countSessionIds(["default", "default", "unique"]);
    expect(matchesStoredSessionId("default", "workspace-a", "default", counts.get("default") ?? 1)).toBe(false);
    expect(matchesStoredSessionId("unique", "workspace-a", "unique", counts.get("unique") ?? 1)).toBe(true);
    expect(matchesStoredSessionId(sidebarSessionKey("workspace-b", "default"), "workspace-b", "default", 2)).toBe(true);
  });

  it("置顶写入复合键并清掉裸 ID", () => {
    expect(nextPinnedIds(["default"], "workspace-a", "default", false)).toEqual([
      sidebarSessionKey("workspace-a", "default")
    ]);
    expect(nextPinnedIds([sidebarSessionKey("workspace-a", "default"), "default"], "workspace-a", "default", true)).toEqual([]);
  });
});
