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
  runs: LiveRunState[],
  sessionId?: string
): ConversationDisplayProjection {
  const historyById = new Map(turns.map((turn) => [turn.turn_id, turn]));
  // 1. 【会话同步】【补发过滤】补发只补齐当前分支运行中的正文，不复活旧分支或窗口之外的历史
  const sessionRuns = runs.filter((run) => {
    if (sessionId && run.sessionId !== sessionId) return false;
    const history = run.runId ? historyById.get(run.runId) : undefined;
    const secretCount = run.parts.filter((part) => part.type === "ssh_secret" && !part.resolved).length;
    const kept = !(history && history.status !== "running") && (!run.replayed || history?.status === "running");
    // #region agent log
    if (secretCount > 0 || run.status === "waiting_ssh_secret") {
      fetch("http://127.0.0.1:7368/ingest/77461c80-9be3-44e4-ac14-3725f6920049",{method:"POST",headers:{"Content-Type":"application/json","X-Debug-Session-Id":"ff618c"},body:JSON.stringify({sessionId:"ff618c",hypothesisId:"C",location:"conversation-display.ts:filter",message:"ssh wait run projection",data:{kept,replayed:run.replayed,completed:run.completed,status:run.status,historyStatus:history?.status??null,secretCount,runId:run.runId??null},timestamp:Date.now()})}).catch(()=>{});
    }
    // #endregion
    if (history && history.status !== "running") return false;
    return !run.replayed || history?.status === "running";
  });
  const livePreferredIds = new Set(
    sessionRuns
      .filter((run) => {
        if (!run.runId) return false;
        const history = historyById.get(run.runId);
        return !run.completed || history?.status === "running";
      })
      .map((run) => run.runId as string)
  );

  // 1. 【前端性能】【历史投影】实时轮次没有替换历史时保留数组引用，使历史派生缓存继续生效
  const replacesHistory = [...livePreferredIds].some((id) => historyById.has(id));
  return {
    historyTurns: replacesHistory ? turns.filter((turn) => !livePreferredIds.has(turn.turn_id)) : turns,
    liveRuns: sessionRuns.filter((run) => {
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
