import type { HistoryEntry, SessionTimelineTurn, TimelineToolEntry } from "../../api/contracts";
import type { LiveRunState } from "./run-event-reducer";
import type { LiveMessagePart } from "./run-event-reducer";
import { LiveRunIndicator, formatTurnElapsed } from "./live-run-indicator";
import { MessageActions } from "./message/message-actions";
import { MessageParts } from "./message/message-parts";
import { UserMessageBubble } from "./message/user-message-bubble";
import { RunErrorNotice } from "./message/run-error-notice";
import { useI18n } from "../i18n/use-i18n";
import { collectTurnFileChanges } from "./turn-changes/collect-turn-file-changes";
import { TurnFileChanges } from "./turn-changes/turn-file-changes";
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

/**
 * 渲染一个包含结构化工具历史的完整对话轮次。
 *
 * @param props turn 为会话时间线轮次，onRetry 为可选的重试本轮回调，仅最后一轮传入
 * @returns 用户消息、工具调用和助手消息
 */
export function HistoryTurn({
  turn,
  sessionId,
  onRetry,
  onFork,
  actionBusy
}: {
  turn: SessionTimelineTurn;
  sessionId?: string | null;
  onRetry?: () => void;
  onFork?: () => void;
  actionBusy?: boolean;
}) {
  const { t, locale } = useI18n();
  // 失败轮仅有错误摘要时不把错误再当正文渲染一遍
  const failureOnly =
    turn.status === "failed"
    && !turn.assistant.reasoning
    && turn.tools.length === 0;
  return (
    <>
      {!turn.automatic && (
        <UserMessageBubble content={turn.user.content} timestamp={turn.user.timestamp} imageUrls={turn.user.image_urls} onRetry={onRetry} />
      )}
      <article className="message assistant-message">
        {!failureOnly && <MessageParts parts={historyTurnParts(turn)} />}
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
            onRetry={onRetry}
          />
        )}
        {typeof turn.duration_ms === "number" && turn.duration_ms > 0 && (
          <div className="turn-duration-meta" role="status">
            {locale.startsWith("zh")
              ? `处理用时${formatTurnElapsed(turn.duration_ms, true)}`
              : `Processing time ${formatTurnElapsed(turn.duration_ms, false)}`}
          </div>
        )}
        <TurnFileChanges
          changes={collectTurnFileChanges(turn.tools)}
          tools={turn.tools}
          sessionId={sessionId}
          turnId={turn.turn_id}
        />
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
export function LiveRunMessage({
  state,
  sessionId,
  running,
  onRetry
}: {
  state: LiveRunState;
  sessionId?: string | null;
  running: boolean;
  onRetry?: () => void;
}) {
  const { t, locale } = useI18n();
  const compacting = state.parts.some((part) => part.type === "compaction" && part.status === "running");
  const queued = state.status === "queued";
  if (queued) return null;
  return (
    <>
      {(state.userInput || state.imageUrls.length > 0) && (
        <UserMessageBubble
          content={state.userInput}
          imageUrls={state.imageUrls}
          onRetry={running ? undefined : onRetry}
        />
      )}
      <article className="message assistant-message live-message">
          <MessageParts parts={state.parts} live={running} />
          {running && !compacting && <LiveRunIndicator status={state.status} startedAtMs={state.startedAtMs} />}
          {!running && state.completed && (
            <>
              {state.durationMs != null && state.durationMs > 0 && (
                <div className="turn-duration-meta" role="status">
                  {locale.startsWith("zh")
                    ? `处理用时${formatTurnElapsed(state.durationMs, true)}`
                    : `Processing time ${formatTurnElapsed(state.durationMs, false)}`}
                </div>
              )}
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
          {!running && state.content && <MessageActions text={state.content} />}
      </article>
    </>
  );
}

/**
 * 从中断轮次提取可展示详情。
 *
 * 1. 优先使用最后一个失败/中断工具的错误或输出
 * 2. 无工具错误时回退到用户主动中断的说明，避免只显示标题
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
    "The user stopped this run before it completed.",
    "用户在运行完成前主动停止了本轮。"
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
  for (let index = turn.tools.length - 1; index >= 0; index -= 1) {
    const tool = turn.tools[index];
    const detail = tool.error?.trim() || (tool.status === "failed" ? tool.output.trim() : "");
    if (detail) return detail;
  }
  if (turn.assistant.content?.trim()) return turn.assistant.content.trim();
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
