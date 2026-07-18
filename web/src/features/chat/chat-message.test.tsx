import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { SessionTimelineTurn } from "../../api/contracts";
import { HistoryTurn } from "./chat-message";

describe("HistoryTurn", () => {
  it("restores a persisted permission card before its historical tool", () => {
    const turn: SessionTimelineTurn = {
      turn_id: "turn",
      seq: 1,
      status: "completed",
      automatic: false,
      user: { timestamp: "now", content: "修改文件" },
      assistant: { timestamp: "later", content: "已保留文件" },
      tools: [{
        id: "call",
        name: "edit_file",
        arguments: "{\"path\":\"src/main.rs\"}",
        status: "failed",
        output: "保留文件",
        created_at: "now",
        permission: { decision: "deny", reply: "保留文件" }
      }]
    };

    const html = renderToStaticMarkup(<HistoryTurn turn={turn} />);

    expect(html).toContain("已拒绝");
    expect(html).toContain("保留文件");
    expect(html.indexOf("已拒绝")).toBeLessThan(html.indexOf("Edit"));
  });

  it("renders an interruption notice for a durable interrupted turn", () => {
    const turn: SessionTimelineTurn = {
      turn_id: "run-1",
      seq: 1,
      status: "interrupted",
      automatic: false,
      user: { timestamp: "now", content: "执行检查" },
      assistant: { timestamp: "later", content: "" },
      tools: []
    };

    const html = renderToStaticMarkup(<HistoryTurn turn={turn} />);

    expect(html).toContain("运行已中断");
  });

  it("hides the internal goal continuation prompt", () => {
    const turn: SessionTimelineTurn = {
      turn_id: "goal-turn",
      seq: 2,
      status: "completed",
      automatic: true,
      user: { timestamp: "now", content: "<goal-continuation>internal</goal-continuation>" },
      assistant: { timestamp: "later", content: "继续完成目标" },
      tools: []
    };

    const html = renderToStaticMarkup(<HistoryTurn turn={turn} />);

    expect(html).not.toContain("goal-continuation");
    expect(html).toContain("继续完成目标");
  });
});
