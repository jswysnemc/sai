import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { SessionTimelineTurn } from "../../api/contracts";
import { HistoryTurn, LiveRunMessage } from "./chat-message";
import { initialRunState } from "./run-event-reducer";
import { UserMessageBubble } from "./message/user-message-bubble";

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

  it("offers expandable details for live failures", () => {
    const html = renderToStaticMarkup(
      <LiveRunMessage
        running={false}
        state={{
          ...initialRunState,
          completed: true,
          error: "运行失败",
          errorDetail: "upstream request timed out after 120 seconds"
        }}
      />
    );

    expect(html).toContain("查看错误详情");
  });

  it("offers the last failed tool output as interruption details", () => {
    const turn: SessionTimelineTurn = {
      turn_id: "run-timeout",
      seq: 2,
      status: "interrupted",
      automatic: false,
      user: { timestamp: "now", content: "执行检查" },
      assistant: { timestamp: "later", content: "" },
      tools: [{
        id: "timeout",
        name: "run_command",
        arguments: "{}",
        status: "failed",
        output: "command timed out after 30 seconds",
        created_at: "now"
      }]
    };

    const html = renderToStaticMarkup(<HistoryTurn turn={turn} />);

    expect(html).toContain("查看错误详情");
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

  it("keeps images and special skill rendering inside the user bubble", () => {
    const html = renderToStaticMarkup(
      <UserMessageBubble
        content={'使用 <skill-reference name="research">\n# Research\nRead primary sources\n</skill-reference> 完成分析'}
        imageUrls={["data:image/png;base64,AA=="]}
      />
    );

    expect(html).toContain('<div class="user-bubble"><div class="user-attachments">');
    expect(html).toContain("user-skill-atom");
    expect(html).toContain("/research");
    expect(html).not.toContain("Read primary sources");
    expect(html).toContain('</div><div class="message-actions user-message-actions">');
  });
});
