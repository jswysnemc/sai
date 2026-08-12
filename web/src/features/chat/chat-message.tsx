import { memo, useMemo, type ReactNode } from "react";
import type { HistoryEntry, SessionTimelineTurn, TimelineToolEntry } from "../../api/contracts";
import type { SessionTurnTree } from "../../api/turn-tree-contracts";
import type { LiveRunState } from "./run-event-reducer";
import type { LiveMessagePart } from "./run-event-reducer";
import { LiveRunIndicator } from "./live-run-indicator";
import { MessageActions } from "./message/message-actions";
import { MessageParts } from "./message/message-parts";
import { UserMessageBubble } from "./message/user-message-bubble";
import { RunErrorNotice } from "./message/run-error-notice";
import { useI18n } from "../i18n/use-i18n";
import { collectTurnFileChanges } from "./turn-changes/collect-turn-file-changes";
import { TurnFileChanges } from "./turn-changes/turn-file-changes";
import { TurnMetrics } from "./message/turn-metrics";
import { BranchSwitcher } from "./turn-tree/branch-switcher";
import "./turn-duration-meta.css";

/**
 * 渲染一条历史消息。
 *
 * @param props 历史消息内容
 * @returns 用户或助手消息
 */
export function HistoryMessage({ message }: { message: HistoryEntry }) {
  if (message.role === "user") return <UserMessageBubble content={message.content} timestamp={message.timestamp} />;
  return (
    <article className="message assistant-message">
      <MessageParts parts={historyMessageParts(message)} />
      {message.content && <MessageActions text={message.content} timestamp={message.timestamp} />}
    </article>
  );
}

type HistoryTurnProps = {
  turn: SessionTimelineTurn;
  sessionId?: string | null;
  canRetry?: boolean;
  onRetry?: (content: string, imageUrls: string[] | undefined, turnId: string | null) => void;
  canContinueFrom?: boolean;
  onContinueFrom?: (turnId: string) => void;
  canSideConversation?: boolean;
  onSideConversation?: (turnId: string) => void;
  canEditResend?: boolean;
  onEditResend?: (turnId: string | null, content: string, imageUrls: string[]) => void;
  actionBusy?: boolean;
  branchTree?: SessionTurnTree;
  branchBusy?: boolean;
  onSwitchBranch?: (turnId: string) => void;
};

/**
 * 比较历史轮次 props：以 turn 引用与能力开关为准，忽略回调身份，
 * 避免父组件每次流式更新都击穿 memo。
 *
 * @param previous 上次 props
 * @param next 本次 props
 * @returns 是否可跳过重渲染
 */
function historyTurnPropsEqual(previous: HistoryTurnProps, next: HistoryTurnProps): boolean {
  return previous.turn === next.turn
    && previous.sessionId === next.sessionId
    && previous.canRetry === next.canRetry
    && previous.canContinueFrom === next.canContinueFrom
    && previous.canSideConversation === next.canSideConversation
    && previous.canEditResend === next.canEditResend
    && previous.actionBusy === next.actionBusy
    && previous.branchTree === next.branchTree
    && previous.branchBusy === next.branchBusy;
}

/**
 * 渲染一个包含结构化工具历史的完整对话轮次。
 *
 * @param props 轮次数据与稳定动作开关
 * @returns 用户消息、工具调用和助手消息
 */
export const HistoryTurn = memo(function HistoryTurn({
  turn,
  sessionId,
  canRetry,
  onRetry,
  canContinueFrom,
  onContinueFrom,
  canSideConversation,
  onSideConversation,
  canEditResend,
  onEditResend,
  actionBusy,
  branchTree,
  branchBusy,
  onSwitchBranch
}: HistoryTurnProps) {
  const { t } = useI18n();
  const parts = useMemo(() => historyTurnParts(turn), [turn]);
  const fileChanges = useMemo(() => collectTurnFileChanges(turn.tools), [turn.tools]);
  // 失败轮仅有错误摘要时不把错误再当正文渲染一遍
  const failureOnly =
    turn.status === "failed"
    && !turn.assistant.reasoning
    && turn.tools.length === 0;
  const branchSlot: ReactNode = onSwitchBranch ? (
    <BranchSwitcher
      tree={branchTree}
      turnId={turn.turn_id}
      busy={branchBusy}
      onSwitch={onSwitchBranch}
    />
  ) : null;
  return (
    <>
      {!turn.automatic && (
        <UserMessageBubble
          content={turn.user.content}
          timestamp={turn.user.timestamp}
          imageUrls={turn.user.image_urls}
          onEditResend={canEditResend && onEditResend
            ? (content, imageUrls) => onEditResend(turn.turn_id, content, imageUrls)
            : undefined}
          actionBusy={actionBusy}
        />
      )}
      <article className="message assistant-message">
        {!failureOnly && <MessageParts parts={parts} />}
        {turn.status === "interrupted" && (
          <RunErrorNotice
            message={turn.assistant.content ? t("The response was interrupted; generated content was preserved", "响应已中断，已保留生成内容") : t("The run was interrupted", "运行已中断")}
            detail={historicalInterruptionDetail(turn, t)}
          />
        )}
        {turn.status === "failed" && (
          <RunErrorNotice
            message={t("The run failed", "运行失败")}
            detail={historicalFailureDetail(turn, t)}
            onRetry={canRetry && onRetry
              ? () => onRetry(turn.user.content, turn.user.image_urls, turn.turn_id)
              : undefined}
          />
        )}
        <TurnMetrics durationMs={turn.duration_ms} usage={turn.usage} />
        <TurnFileChanges
          changes={fileChanges}
          tools={turn.tools}
          sessionId={sessionId}
          turnId={turn.turn_id}
        />
        {(turn.assistant.content || canRetry || branchSlot || canSideConversation) && (
          <MessageActions
            text={turn.assistant.content || turn.user.content}
            timestamp={turn.assistant.timestamp}
            onRetry={canRetry && onRetry
              ? () => onRetry(turn.user.content, turn.user.image_urls, turn.turn_id)
              : undefined}
            onContinueFrom={canContinueFrom && onContinueFrom
              ? () => onContinueFrom(turn.turn_id)
              : undefined}
            onSideConversation={canSideConversation && onSideConversation
              ? () => onSideConversation(turn.turn_id)
              : undefined}
            busy={actionBusy}
            extra={branchSlot}
          />
        )}
      </article>
    </>
  );
}, historyTurnPropsEqual);

/**
 * 渲染当前正在流式生成的用户输入和助手回复。
 *
 * @param props state 为运行状态，running 为运行标记
 * @returns 当前运行消息组
 */
export const LiveRunMessage = memo(function LiveRunMessage({
  state,
  sessionId,
  running,
  onRetry,
  onEditResend,
  actionBusy
}: {
  state: LiveRunState;
  sessionId?: string | null;
  running: boolean;
  onRetry?: () => void;
  onEditResend?: (content: string, imageUrls: string[]) => void;
  actionBusy?: boolean;
}) {
  const { t } = useI18n();
  const compacting = state.parts.some((part) => part.type === "compaction" && part.status === "running");
  const queued = state.status === "queued";
  if (queued) {
    return (
      <UserMessageBubble
        content={state.userInput}
        imageUrls={state.imageUrls}
      />
    );
  }
  return (
    <>
      {(state.userInput || state.imageUrls.length > 0) && (
        <UserMessageBubble
          content={state.userInput}
          imageUrls={state.imageUrls}
          onEditResend={running ? undefined : onEditResend}
          actionBusy={actionBusy}
        />
      )}
      <article className="message assistant-message live-message">
          <MessageParts parts={state.parts} live={running} />
          {running && !compacting && (
            <LiveRunIndicator
              status={state.status}
              startedAtMs={state.startedAtMs}
              reconnectAttempt={state.reconnectAttempt}
              reconnectMaxAttempts={state.reconnectMaxAttempts}
            />
          )}
          {!running && state.completed && (
            <>
              <TurnMetrics durationMs={state.durationMs} usage={state.usage} />
              <TurnFileChanges
                changes={collectTurnFileChanges(state.tools)}
                tools={state.tools}
                sessionId={sessionId}
                turnId={state.runId}
              />
            </>
          )}
          {state.error && (
            <RunErrorNotice
              message={state.error}
              detail={state.errorDetail}
              onRetry={onRetry && state.completed ? onRetry : undefined}
            />
          )}
          {!running && (state.content || onRetry) && (
            <MessageActions text={state.content} onRetry={onRetry} busy={actionBusy} />
          )}
      </article>
    </>
  );
});

/**
 * 从中断轮次提取可展示详情。
 *
 * @param turn 已持久化会话轮次
 * @param t 本地化函数
 * @returns 详情文本
 */
function historicalInterruptionDetail(
  turn: SessionTimelineTurn,
  t: (en: string, zh: string) => string
): string {
  for (let index = turn.tools.length - 1; index >= 0; index -= 1) {
    const tool = turn.tools[index];
    const detail = tool.error?.trim() || (tool.status === "failed" ? tool.output.trim() : "");
    if (detail) return detail;
  }
  if (turn.assistant.content?.trim()) {
    return t(
      "Generation stopped before the response finished. Partial content above was preserved.",
      "生成在完整结束前被停止，上方已保留部分内容。"
    );
  }
  if (turn.tools.length > 0) {
    return t(
      "The run was stopped while tools were still in progress. Pending tool calls may be incomplete.",
      "运行在工具执行过程中被停止，未完成的工具调用可能未写回结果。"
    );
  }
  return t(
    "The run was interrupted before completion; no confirmed cause was reported.",
    "运行在完成前中断，未收到可确认的中断原因。"
  );
}

/**
 * 从失败轮次提取可展示详情。
 *
 * @param turn 已持久化会话轮次
 * @param t 本地化函数
 * @returns 详情文本
 */
function historicalFailureDetail(
  turn: SessionTimelineTurn,
  t: (en: string, zh: string) => string
): string {
  // 落库的失败原因最准确；正文已在消息气泡渲染，绝不当错误详情用
  if (turn.error?.trim()) return turn.error.trim();
  for (let index = turn.tools.length - 1; index >= 0; index -= 1) {
    const tool = turn.tools[index];
    const detail = tool.error?.trim() || (tool.status === "failed" ? tool.output.trim() : "");
    if (detail) return detail;
  }
  return t("The provider request failed before a response was produced.", "供应商请求在产生回复前失败。");
}

/**
 * 将旧版消息转换为统一消息部件。
 *
 * @param message 历史消息
 * @returns 有序消息部件
 */
function historyMessageParts(message: HistoryEntry): LiveMessagePart[] {
  const parts: LiveMessagePart[] = [];
  if (message.reasoning) parts.push({ id: `reasoning-${message.timestamp}`, type: "reasoning", source: message.reasoning, startedAt: "" });
  if (message.content) parts.push({ id: `text-${message.timestamp}`, type: "text", source: message.content });
  return parts;
}

/**
 * 将会话轮次转换为同一消息内的有序部件。
 *
 * @param turn 会话时间线轮次
 * @returns 思考、工具和正文部件
 */
function historyTurnParts(turn: SessionTimelineTurn): LiveMessagePart[] {
  const parts: LiveMessagePart[] = [];
  const messages = [...(turn.messages ?? [])].sort((left, right) => left.seq - right.seq);
  let messageIndex = 0;

  /** 追加不晚于指定工具调用序号的轮次内消息。 */
  const appendMessagesThrough = (toolSeq: number) => {
    while (messageIndex < messages.length && messages[messageIndex].after_tool_seq <= toolSeq) {
      const message = messages[messageIndex];
      if (message.reasoning) {
        parts.push({
          id: `${message.id}-reasoning`,
          type: "reasoning",
          source: message.reasoning,
          startedAt: message.created_at,
          endedAt: message.created_at
        });
      }
      if (message.content) {
        parts.push(message.role === "assistant"
          ? { id: `${message.id}-text`, type: "text", source: message.content }
          : { id: `${message.id}-input`, type: "automatic_input", kind: message.kind, source: message.content });
      }
      messageIndex += 1;
    }
  };

  appendMessagesThrough(0);
  const tools = [...turn.tools].sort((left, right) => {
    const sequenceOrder = (left.seq ?? Number.MAX_SAFE_INTEGER) - (right.seq ?? Number.MAX_SAFE_INTEGER);
    return sequenceOrder || left.created_at.localeCompare(right.created_at);
  });
  for (const tool of tools) {
    if (tool.permission) {
      parts.push({
        id: `${turn.turn_id}-${tool.id}-permission`,
        type: "permission",
        request: {
          id: `history-${tool.id}`,
          session_id: "",
          tool: tool.name,
          arguments: tool.arguments
        },
        decision: tool.permission
      });
    }
    parts.push({ id: `${turn.turn_id}-${tool.id}`, type: "tool", tool: timelineTool(tool) });
    appendMessagesThrough(tool.seq ?? Number.MAX_SAFE_INTEGER);
  }
  appendMessagesThrough(Number.MAX_SAFE_INTEGER);
  if (turn.assistant.reasoning) {
    parts.push({ id: `${turn.turn_id}-reasoning`, type: "reasoning", source: turn.assistant.reasoning, startedAt: "" });
  }
  if (turn.assistant.content) parts.push({ id: `${turn.turn_id}-text`, type: "text", source: turn.assistant.content });
  return parts;
}

/**
 * 将后端时间线工具记录转换为统一生命周期状态。
 *
 * @param tool 时间线工具记录
 * @returns 工具生命周期状态
 */
function timelineTool(tool: TimelineToolEntry): LiveRunState["tools"][number] {
  const startedAtMs = Date.parse(tool.created_at);
  const endedAtMs = tool.completed_at ? Date.parse(tool.completed_at) : Number.NaN;
  return {
    id: tool.id,
    name: tool.name,
    argumentsPreview: tool.arguments,
    arguments: tool.arguments,
    progress: "",
    output: tool.output || tool.error || "",
    status: tool.status,
    startedAtMs: Number.isNaN(startedAtMs) ? undefined : startedAtMs,
    endedAtMs: Number.isNaN(endedAtMs) ? undefined : endedAtMs
  };
}
