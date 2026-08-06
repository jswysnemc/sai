import { afterEach, describe, expect, it, vi } from "vitest";
import type { SessionTimelineTurn } from "../../api/contracts";
import { composeSideConversationInput, createSideConversationRequest } from "./side-conversation-context";

afterEach(() => vi.unstubAllGlobals());

/**
 * 创建旁路上下文测试使用的会话轮次。
 *
 * @param id 轮次标识
 * @param user 用户正文
 * @param assistant 助手正文
 * @param status 轮次状态
 * @returns 完整会话轮次
 */
function turn(
  id: string,
  user: string,
  assistant: string,
  status: SessionTimelineTurn["status"] = "completed"
): SessionTimelineTurn {
  return {
    turn_id: id,
    seq: 1,
    status,
    automatic: false,
    user: { content: user, timestamp: "" },
    assistant: { content: assistant, timestamp: "" },
    tools: []
  };
}

describe("side conversation context", () => {
  it("默认选择最后一个已完成助手回复并排除生成中内容", () => {
    vi.stubGlobal("crypto", { randomUUID: () => "side-id" });
    const request = createSideConversationRequest({
      turns: [
        turn("one", "第一个问题", "第一个回答"),
        turn("two", "第二个问题", "尚未完成", "running")
      ],
      workspaceId: "workspace-1",
      sourceSessionId: "session-1"
    });

    expect(request?.sourceTurnId).toBe("one");
    expect(request?.context).toContain("第一个回答");
    expect(request?.context).not.toContain("尚未完成");
  });

  it("指定历史回复时只携带该回复及此前上下文", () => {
    vi.stubGlobal("crypto", { randomUUID: () => "side-id" });
    const request = createSideConversationRequest({
      turns: [
        turn("one", "第一个问题", "第一个回答"),
        turn("two", "第二个问题", "第二个回答")
      ],
      sourceTurnId: "one",
      workspaceId: "workspace-1",
      sourceSessionId: "session-1"
    });

    expect(request?.context).toContain("第一个问题");
    expect(request?.context).not.toContain("第二个问题");
  });

  it("首轮模型输入包装冻结上下文但保留单独展示问题", () => {
    const input = composeSideConversationInput("主会话内容", "这里是什么意思？");

    expect(input).toContain("<side_context>\n主会话内容\n</side_context>");
    expect(input.endsWith("这里是什么意思？")).toBe(true);
  });
});
