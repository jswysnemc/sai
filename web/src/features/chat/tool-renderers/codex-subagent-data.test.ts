import { describe, expect, it } from "vitest";
import { parseCodexSubagentActivity } from "./codex-subagent-data";

describe("Codex subagent data", () => {
  it("parses native activity fields and derives the agent name", () => {
    expect(parseCodexSubagentActivity(JSON.stringify({
      agentThreadId: "thread-audit",
      agentPath: "/root/audit",
      activityKind: "started"
    }))).toEqual({
      threadId: "thread-audit",
      path: "/root/audit",
      name: "audit",
      activity: "started"
    });
  });

  it("falls back to preserved ACP metadata", () => {
    expect(parseCodexSubagentActivity(JSON.stringify({
      _acp: {
        meta: {
          codex: {
            subagent: {
              threadId: "thread-review",
              path: "/root/review",
              activity: "interacted"
            }
          }
        }
      }
    }))).toEqual({
      threadId: "thread-review",
      path: "/root/review",
      name: "review",
      activity: "interacted"
    });
  });

  it("rejects unrelated or unsupported activity payloads", () => {
    expect(parseCodexSubagentActivity("{}" )).toBeNull();
    expect(parseCodexSubagentActivity(JSON.stringify({
      agentThreadId: "thread-audit",
      agentPath: "/root/audit",
      activityKind: "completed"
    }))).toBeNull();
  });
});
