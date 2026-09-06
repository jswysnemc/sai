import { apiRequest } from "./client";

export type SessionTurnPreview = {
  turn_id: string;
  user: string;
  assistant: string;
  messages: { id: string; role: string; content: string }[];
};

/**
 * 读取指定分支轮次的完整正文，不改变当前分支。
 * @param sessionId 会话标识
 * @param turnId 轮次标识
 * @returns 完整用户输入、回复及轮次内消息
 */
export function fetchSessionTurnPreview(sessionId: string, turnId: string): Promise<SessionTurnPreview> {
  return apiRequest(`/api/sessions/${encodeURIComponent(sessionId)}/turn-tree/${encodeURIComponent(turnId)}`);
}
