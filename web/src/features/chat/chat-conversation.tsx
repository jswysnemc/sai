import { Fragment, useMemo, type ComponentProps } from "react";
import type { SessionTimelineTurn } from "../../api/contracts";
import { HistoryTurn, LiveRunMessage } from "./chat-message";
import { deriveModelSwitchMarkers } from "./model-switch-divider";
import { ModelSwitchDivider } from "./message/model-switch-divider";
import type { LiveRunState } from "./run-event-reducer";

type HistoryActions = Omit<ComponentProps<typeof HistoryTurn>, "turn" | "canRetry" | "canSideConversation">;
type ChatConversationProps = {
  turns: SessionTimelineTurn[];
  liveRuns: LiveRunState[];
  running: boolean;
  lastTurnId?: string;
  actions: HistoryActions;
};

/**
 * 渲染历史与流式消息，并保持分支、重试和模型切换标记一致。
 * @param props 消息集合、运行状态和会话动作
 * @returns 有序的对话轮次
 */
export function ChatConversation({ turns, liveRuns, running, lastTurnId, actions }: ChatConversationProps) {
  const markers = useMemo(() => deriveModelSwitchMarkers([
    ...turns.map((turn) => ({ key: turn.turn_id, model: turn.model })),
    ...liveRuns.filter((state) => state.runId).map((state) => ({ key: state.runId!, model: state.model }))
  ]), [turns, liveRuns]);
  return (
    <>
      {turns.map((turn) => {
        const marker = markers.get(turn.turn_id);
        return (
          <Fragment key={turn.turn_id}>
            {marker && <ModelSwitchDivider marker={marker} />}
            <section className="conversation-turn" data-overview-id={`turn-${turn.turn_id}`}>
              <HistoryTurn {...actions} turn={turn} canRetry={turn.turn_id === lastTurnId && !running} canSideConversation={turn.status === "completed" && Boolean(turn.assistant.content.trim())} />
            </section>
          </Fragment>
        );
      })}
      {liveRuns.map((state) => {
        const marker = state.runId ? markers.get(state.runId) : undefined;
        return (
          <Fragment key={state.runId}>
            {marker && <ModelSwitchDivider marker={marker} />}
            <section className="conversation-turn" data-overview-id={`live-${state.runId}`}>
              <LiveRunMessage
                state={state}
                sessionId={actions.sessionId}
                running={!state.completed}
                onRetry={!running && state.completed ? () => actions.onRetry?.(state.userInput, state.imageUrls, state.runId) : undefined}
                onEditResend={!running && state.completed ? (content, imageUrls) => actions.onEditResend?.(state.runId, content, imageUrls) : undefined}
                actionBusy={actions.actionBusy}
              />
            </section>
          </Fragment>
        );
      })}
    </>
  );
}
