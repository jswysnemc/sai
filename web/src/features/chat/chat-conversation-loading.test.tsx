import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { SessionTimelineTurn } from "../../api/contracts";
import { ChatConversation } from "./chat-conversation";

vi.mock("./chat-message", () => ({
  HistoryTurn: ({ turn }: { turn: SessionTimelineTurn }) => <p data-rendered-turn={turn.turn_id}>{turn.assistant.content}</p>,
  LiveRunMessage: () => <p data-live-message />,
}));

/**
 * 【会话载入】【历史样本】构造指定数量的完整历史轮次。
 * @param count 历史轮数
 * @returns 按时间顺序排列的轮次
 */
function history(count: number): SessionTimelineTurn[] {
  return Array.from({ length: count }, (_, index) => ({
    turn_id: `turn-${index + 1}`,
    seq: index + 1,
    status: "completed",
    user: { content: `question-${index + 1}`, timestamp: "", reasoning: null },
    assistant: { content: `answer-${index + 1}`, timestamp: "", reasoning: null },
    tools: [],
    automatic: false,
  }));
}

describe("conversation history loading", () => {
  it("mounts a bounded recent tail while retaining every history navigation anchor", () => {
    const html = renderToStaticMarkup(<ChatConversation turns={history(500)} liveRuns={[]} running={false} actions={{}} />);
    expect(html.match(/data-overview-id="turn-/g)).toHaveLength(500);
    expect(html.match(/data-rendered-turn=/g)!.length).toBeLessThanOrEqual(12);
    expect(html).toContain("answer-500");
  });

  it("renders short histories completely", () => {
    const html = renderToStaticMarkup(<ChatConversation turns={history(3)} liveRuns={[]} running={false} actions={{}} />);
    expect(html.match(/data-rendered-turn=/g)).toHaveLength(3);
  });
});
