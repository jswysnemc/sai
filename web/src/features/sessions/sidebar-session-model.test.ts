import { describe, expect, it } from "vitest";
import type { Session } from "../../api/contracts";
import { bucketSessions, timelineBucketId, type SidebarSessionRef } from "./sidebar-session-model";

const now = new Date(2026, 8, 23, 12, 0, 0).getTime();

/**
 * 造一条只用于分桶的会话。
 *
 * @param updatedAt 更新时间
 * @returns 侧栏会话
 */
function entry(updatedAt: string): SidebarSessionRef {
  const session: Session = {
    id: updatedAt,
    title: updatedAt,
    created_at: updatedAt,
    updated_at: updatedAt,
    active: false
  };
  return {
    workspaceId: "workspace",
    workspaceName: "Workspace",
    workspacePath: "/tmp/workspace",
    workspaceActive: true,
    session
  };
}

describe("timeline buckets", () => {
  it("把今天、昨天和更早的会话分到固定顺序的桶", () => {
    expect(timelineBucketId("2026-09-23T01:00:00", now)).toBe("today");
    expect(timelineBucketId("2026-09-22T18:00:00", now)).toBe("yesterday");
    const buckets = bucketSessions([
      entry("2026-09-23T08:00:00"),
      entry("2026-09-22T08:00:00"),
      entry("2026-01-02T08:00:00")
    ], now);
    expect(buckets.map((bucket) => bucket.id)).toEqual(["today", "yesterday", "earlier"]);
  });
});
