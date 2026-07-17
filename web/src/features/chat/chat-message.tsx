import { RotateCcw } from "lucide-react";
import type { HistoryEntry, SessionTimelineTurn, TimelineToolEntry } from "../../api/contracts";
import type { LiveRunState } from "./run-event-reducer";
import type { LiveMessagePart } from "./run-event-reducer";
import { LiveRunIndicator } from "./live-run-indicator";
import { MessageActions } from "./message/message-actions";
import { MessageParts } from "./message/message-parts";
import { UserMessageBubble } from "./message/user-message-bubble";
import { ErrorDetailToggle } from "./message/error-detail-toggle";

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

/**
 * 渲染一个包含结构化工具历史的完整对话轮次。
 *
 * @param props turn 为会话时间线轮次，onRetry 为可选的重试本轮回调，仅最后一轮传入
 * @returns 用户消息、工具调用和助手消息
 */
export function HistoryTurn({
  turn,
  onRetry,
  onFork,
  actionBusy
}: {
  turn: SessionTimelineTurn;
  onRetry?: () => void;
  onFork?: () => void;
  actionBusy?: boolean;
}) {
  return (
    <>
      <UserMessageBubble content={turn.user.content} timestamp={turn.user.timestamp} onRetry={onRetry} />
      <article className="message assistant-message">
        <MessageParts parts={historyTurnParts(turn)} />
        {turn.status === "interrupted" && (
          <div className="run-error">
            <span className="run-error-text">
              {turn.assistant.content ? "响应已中断，已保留生成内容" : "运行已中断"}
            </span>
          </div>
        )}
        {(turn.assistant.content || onFork) && (
          <MessageActions
            text={turn.assistant.content || turn.user.content}
            timestamp={turn.assistant.timestamp}
            onFork={onFork}
            busy={actionBusy}
          />
        )}
      </article>
    </>
  );
}

/**
 * 渲染当前正在流式生成的用户输入和助手回复。
 *
 * @param props state 为运行状态，running 为运行标记，onRetry 为可选的重试本轮回调
 * @returns 当前运行消息组
 */
export function LiveRunMessage({ state, running, onRetry }: { state: LiveRunState; running: boolean; onRetry?: () => void }) {
  const compacting = state.parts.some((part) => part.type === "compaction" && part.status === "running");
  return (
    <>
      {(state.userInput || state.imageUrls.length > 0) && (
        <UserMessageBubble content={state.userInput} imageUrls={state.imageUrls} onRetry={running ? undefined : onRetry} />
      )}
      <article className="message assistant-message live-message">
        <MessageParts parts={state.parts} live={running} />
        {running && !compacting && <LiveRunIndicator status={state.status} />}
        {state.error && (
          <div className="run-error">
            <span className="run-error-text">{state.error}</span>
            {state.errorDetail && <ErrorDetailToggle detail={state.errorDetail} />}
            {onRetry && state.completed && (
              <button type="button" className="run-error-retry" onClick={onRetry}>
                <RotateCcw size={12} />
                <span>重试</span>
              </button>
            )}
          </div>
        )}
        {!running && state.content && <MessageActions text={state.content} />}
      </article>
    </>
  );
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
  if (turn.assistant.reasoning) {
    parts.push({ id: `${turn.turn_id}-reasoning`, type: "reasoning", source: turn.assistant.reasoning, startedAt: "" });
  }
  const tools = [...turn.tools].sort((left, right) => left.created_at.localeCompare(right.created_at));
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
  return {
    id: tool.id,
    name: tool.name,
    argumentsPreview: tool.arguments,
    arguments: tool.arguments,
    progress: "",
    output: tool.output || tool.error || "",
    status: tool.status
  };
}
