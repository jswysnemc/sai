import type { SessionTimelineTurn } from "../../api/contracts";
import type { LiveRunState } from "./run-event-reducer";

export type ConversationDisplayProjection = {
  historyTurns: SessionTimelineTurn[];
  liveRuns: LiveRunState[];
};

/**
 * 合并持久化时间线和实时运行，确保每个稳定轮次标识只展示一次。
 *
 * @param turns 服务端持久化时间线
 * @param runs 当前页面接收的实时运行状态
 * @returns 去重后的历史轮次和实时运行
 */
export function projectConversationDisplay(
  turns: SessionTimelineTurn[],
  runs: LiveRunState[]
): ConversationDisplayProjection {
  const historyById = new Map(turns.map((turn) => [turn.turn_id, turn]));
  const livePreferredIds = new Set(
    runs
      .filter((run) => {
        if (!run.runId) return false;
        const history = historyById.get(run.runId);
        return !run.completed || history?.status === "running";
      })
      .map((run) => run.runId as string)
  );

  return {
    historyTurns: turns.filter((turn) => !livePreferredIds.has(turn.turn_id)),
    liveRuns: runs.filter((run) => {
      if (!run.runId || !run.completed) return true;
      const history = historyById.get(run.runId);
      return !history || history.status === "running";
    })
  };
}

/**
 * 返回重试前可以安全回滚的最后一轮标识。
 *
 * @param turns 服务端持久化时间线
 * @param candidateTurnId 被点击消息对应的候选轮次标识
 * @returns 仍位于持久化时间线中的轮次标识
 */
export function retryableTurnId(
  turns: SessionTimelineTurn[],
  candidateTurnId: string | null | undefined
): string | undefined {
  return candidateTurnId && turns.some((turn) => turn.turn_id === candidateTurnId)
    ? candidateTurnId
    : undefined;
}
