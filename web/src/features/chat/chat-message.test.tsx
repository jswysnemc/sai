import { renderWithProviders } from "../../shared/testing/render-with-providers";
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

    const html = renderWithProviders(<HistoryTurn turn={turn} />);

    expect(html).toContain("已拒绝");
    expect(html).toContain("保留文件");
    expect(html.indexOf("已拒绝")).toBeLessThan(html.indexOf("Edit"));
  });

  it("does not attribute a durable interruption to the user without evidence", () => {
    const turn: SessionTimelineTurn = {
      turn_id: "run-1",
      seq: 1,
      status: "interrupted",
      automatic: false,
      user: { timestamp: "now", content: "执行检查" },
      assistant: { timestamp: "later", content: "" },
      tools: []
    };

    const html = renderWithProviders(<HistoryTurn turn={turn} />);

    expect(html).toContain("运行已中断");
    expect(html).toContain("未收到可确认的中断原因");
    expect(html).not.toContain("用户在运行完成前主动停止了本轮");
  });


  it("renders queued runs as user message bubbles", () => {
    const html = renderWithProviders(
      <LiveRunMessage
        running
        state={{
          ...initialRunState,
          status: "queued",
          userInput: "queued task",
          runId: "run-q",
          startedAtMs: null,
          durationMs: null
        }}
      />
    );
    expect(html).toContain("queued task");
    expect(html).toContain('class="message user-message"');
  });

  it("offers expandable details for live failures", () => {
    const html = renderWithProviders(
      <LiveRunMessage
        running={false}
        state={{
          ...initialRunState,
          startedAtMs: null,
    durationMs: null,
          completed: true,
          error: "运行失败",
          errorDetail: "upstream request timed out after 120 seconds"
        }}
      />
    );

    expect(html).toContain("run-error-detail-toggle");
    expect(html).toContain("详情");
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

    const html = renderWithProviders(<HistoryTurn turn={turn} />);

    expect(html).toContain("运行已中断");
    expect(html).toContain("command timed out after 30 seconds");
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

    const html = renderWithProviders(<HistoryTurn turn={turn} />);

    expect(html).not.toContain("goal-continuation");
    expect(html).toContain("继续完成目标");
  });

  it("restores message-gap receipts after the timeline is reloaded", () => {
    const turn: SessionTimelineTurn = {
      turn_id: "message-gap-turn",
      seq: 3,
      status: "completed",
      automatic: false,
      user: { timestamp: "now", content: "执行并行检查" },
      assistant: { timestamp: "later", content: "全部处理完成" },
      tools: [],
      messages: [
        {
          id: "intermediate",
          seq: 1,
          after_tool_seq: 0,
          kind: "assistant",
          role: "assistant",
          content: "已启动子任务",
          created_at: "middle"
        },
        {
          id: "completion",
          seq: 2,
          after_tool_seq: 0,
          kind: "external_completion",
          role: "user",
          content: "子智能体 A 已完成",
          created_at: "middle"
        }
      ]
    };

    const html = renderWithProviders(<HistoryTurn turn={turn} />);

    expect(html).toContain("已启动子任务");
    expect(html).toContain("子智能体 A 已完成");
    expect(html).toContain("全部处理完成");
    expect(html.indexOf("已启动子任务")).toBeLessThan(html.indexOf("子智能体 A 已完成"));
  });

  it("keeps images and special skill rendering inside the user bubble", () => {
    const html = renderWithProviders(
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
