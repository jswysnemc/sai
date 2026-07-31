export type ConversationSnapshot = {
  timelineLoading: boolean;
  historyTurnCount: number;
  liveRunCount: number;
  hasHistoryCompaction: boolean;
};

/**
 * 判断当前会话是否尚未产生任何可展示内容。
 *
 * @param snapshot 时间线加载状态、历史轮次、实时运行和压缩摘要状态
 * @returns 会话是否为空
 */
export function isConversationEmpty(snapshot: ConversationSnapshot): boolean {
  return !snapshot.timelineLoading
    && snapshot.historyTurnCount === 0
    && snapshot.liveRunCount === 0
    && !snapshot.hasHistoryCompaction;
}

/**
 * 判断空会话问候语和输入区是否应显示在页面中央。
 *
 * @param conversationEmpty 当前会话是否为空
 * @param activeSessionId 当前活动会话标识
 * @param submittedSessionId 已经乐观提交首条消息的会话标识
 * @returns 是否使用居中空态布局
 */
export function shouldCenterEmptySession(
  conversationEmpty: boolean,
  activeSessionId: string | undefined,
  submittedSessionId: string | null
): boolean {
  return Boolean(
    conversationEmpty
    && activeSessionId
    && submittedSessionId !== activeSessionId
  );
}
