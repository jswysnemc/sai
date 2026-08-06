import type { RunMode, RunModelSelection, ThinkingLevel } from "../../api/contracts";

export const OPEN_SIDE_CONVERSATION_EVENT = "sai:open-side-conversation";
export const SIDE_CONVERSATION_SESSION_PREFIX = "__sai_side__:";

export type SideConversationRequest = {
  id: string;
  title: string;
  workspaceId: string;
  sourceSessionId: string;
  sourceTurnId: string;
  context: string;
  mode: RunMode;
  selection?: RunModelSelection;
  thinkingLevel: ThinkingLevel;
  agentId?: string;
};

/**
 * 通知工作区为指定上下文新建旁路对话标签。
 *
 * @param request 已构造的临时对话请求
 * @returns 无返回值
 */
export function openSideConversation(request: SideConversationRequest): void {
  window.dispatchEvent(new CustomEvent<SideConversationRequest>(OPEN_SIDE_CONVERSATION_EVENT, { detail: request }));
}

/**
 * 判断持久化会话是否属于旁路对话的内部临时会话。
 *
 * @param title 会话标题
 * @returns 标题是否使用内部前缀
 */
export function isSideConversationSessionTitle(title: string): boolean {
  return title.startsWith(SIDE_CONVERSATION_SESSION_PREFIX);
}
