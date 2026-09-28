import { describe, expect, it } from "vitest";
import type { SessionDataSummary } from "../../../api/contracts";
import { filterSessionData, isIdleSession, sessionKey, toSelection } from "./session-data-selection";

/** 生成会话摘要；参数为覆盖值，返回测试数据。 */
function session(patch: Partial<SessionDataSummary> = {}): SessionDataSummary {
  return { id: "default", workspace_id: "a", workspace_name: "Workspace A", workspace_path: "/work/a", title: "Review", updated_at: "2026-09-29", created_at: "2026-09-28", active: false, total_bytes: 2048, file_count: 1, items: [], ...patch };
}

describe("session data selection", () => {
  it("keeps equal session identifiers separate across workspaces", () => {
    expect(sessionKey(session())).not.toBe(sessionKey(session({ workspace_id: "b" })));
    expect(toSelection(session({ workspace_id: "b" }))).toEqual({ workspace_id: "b", session_id: "default" });
  });
  it("requires explicit idle status regardless of which session is selected", () => {
    expect(isIdleSession(session())).toBe(false);
    expect(isIdleSession(session({ busy: true }))).toBe(false);
    expect(isIdleSession(session({ active: true, busy: false }))).toBe(true);
  });
  it("combines workspace, date and storage filters", () => {
    const rows = [session(), session({ workspace_id: "b" }), session({ total_bytes: 1 })];
    expect(filterSessionData(rows, "2026-09-29", "a", 1000)).toEqual([rows[0]]);
    expect(filterSessionData(rows, "missing", "", 0)).toEqual([]);
  });
});
