import type { RunMode, RunModelSelection, SessionTimelineTurn, ThinkingLevel } from "../../api/contracts";
import type { SideConversationRequest } from "./side-conversation-events";

type CreateSideConversationRequestOptions = {
  turns: readonly SessionTimelineTurn[];
  sourceTurnId?: string;
  workspaceId: string;
  sourceSessionId: string;
  mode?: RunMode;
  selection?: RunModelSelection;
  thinkingLevel?: ThinkingLevel;
  agentId?: string;
};

/**
 * 从指定已完成回复及此前轮次构造临时旁路上下文。
 *
 * @param options 主会话时间线、目标轮次和运行偏好
 * @returns 可打开旁路标签的请求；没有已完成回复时返回 null
 */
export function createSideConversationRequest(
  options: CreateSideConversationRequestOptions
): SideConversationRequest | null {
  const completed = options.turns.filter((turn) => turn.status === "completed" && Boolean(turn.assistant.content.trim()));
  const source = options.sourceTurnId
    ? completed.find((turn) => turn.turn_id === options.sourceTurnId)
    : completed.at(-1);
  if (!source) return null;
  const sourceIndex = options.turns.findIndex((turn) => turn.turn_id === source.turn_id);
  const context = options.turns
    .slice(0, sourceIndex + 1)
    .filter((turn) => !turn.automatic)
    .flatMap((turn) => [
      turn.user.content.trim() ? `## 用户\n\n${turn.user.content.trim()}` : "",
      turn.assistant.content.trim() ? `## 助手\n\n${turn.assistant.content.trim()}` : ""
    ])
    .filter(Boolean)
    .join("\n\n");
  const title = source.assistant.content.trim().split("\n")[0].replace(/^#+\s*/u, "").slice(0, 28) || "旁路对话";
  return {
    id: crypto.randomUUID(),
    title,
    workspaceId: options.workspaceId,
    sourceSessionId: options.sourceSessionId,
    sourceTurnId: source.turn_id,
    context,
    mode: options.mode ?? "yolo",
    selection: options.selection,
    thinkingLevel: options.thinkingLevel ?? "auto",
    agentId: options.agentId
  };
}

/**
 * 把旁路来源上下文和首个问题组合成只发送一次的模型输入。
 *
 * @param context 截止目标回复的主会话上下文
 * @param question 用户在旁路对话中的问题
 * @returns 发给模型的完整首轮输入
 */
export function composeSideConversationInput(context: string, question: string): string {
  return [
    "以下内容来自主会话的临时只读上下文。请只回答本次疑问，不要假设需要继续执行主会话任务。",
    "<side_context>",
    context,
    "</side_context>",
    "",
    question
  ].join("\n");
}
