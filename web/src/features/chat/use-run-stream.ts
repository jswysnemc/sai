import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useReducer, useRef } from "react";
import type { RunMode, RunModelSelection, ThinkingLevel, WebEvent } from "../../api/contracts";
import { api } from "../../api/client";
import { runFailureEvent } from "./run-failure-event";
import { parseQueueInsertAt, type LiveRunState } from "./run-event-reducer";
import { useI18n } from "../i18n/use-i18n";
import { text } from "../i18n/locale";
import { createReplyNotifier } from "../../shared/notify/reply-notification";
import { initialSessionRunsState, sessionRunsReducer, type HistoryTurnKey, type SessionRunsAction, type SessionRunsState } from "./session-runs-reducer";
export { applyEventsToSessionRuns, sessionRunsReducer, upsertRunFromEvent, updateQueuedRunState } from "./session-runs-reducer";
export type { HistoryTurnKey } from "./session-runs-reducer";

const EVENT_TYPES = [
  "run.queued",
  "run.queue.updated",
  "run.merged",
  "run.dequeued",
  "run.started",
  "message.automatic.input",
  "status.changed",
  "message.content.delta",
  "message.reasoning.delta",
  "context.updated",
  "tool.call.preparing",
  "tool.call.started",
  "tool.progress",
  "tool.result",
  "permission.requested",
  "permission.resolved",
  "question.requested",
  "question.resolved",
  "workspace.changed",
  "content.flushed",
  "engine.ready",
  "compaction.started",
  "compaction.delta",
  "compaction.finished",
  "loaded_tools.changed",
  "session.summary",
  "session.renamed",
  "run.completed",
  "run.interrupted",
  "run.failed",
  "stream.lagged"
] as const;

/** 高频流式事件：合并到同一动画帧再进 reducer，降低 React 提交次数 */
const COALESCED_EVENT_TYPES = new Set<WebEvent["type"]>([
  "message.content.delta",
  "message.reasoning.delta",
  "tool.progress",
  "compaction.delta",
]);

/**
 * 管理一个会话中的活动和排队 Agent 运行。
 *
 * @param workspaceId 当前工作区标识
 * @param sessionId 当前会话标识
 * @param onSettled 运行结束回调
 * @param onWorkspaceChanged 工作区文件变化回调
 * @param onInterruptedWithoutReply 无回复中断输入恢复回调
 * @param onQueueMerged 排队消息并入当前轮时的回调
 * @returns 会话运行状态与启动、停止、重置操作
 */
export function useRunStream(
  workspaceId: string | undefined,
  sessionId: string | undefined,
  onSettled: () => void,
  onWorkspaceChanged?: () => void,
  onInterruptedWithoutReply?: (input: string) => void,
  onQueueMerged?: (input: string) => void
) {
  const { locale } = useI18n();
  const queryClient = useQueryClient();
  const reducer = useCallback(
    (state: SessionRunsState, action: SessionRunsAction) => sessionRunsReducer(state, action, locale),
    [locale]
  );
  const [state, dispatch] = useReducer(reducer, initialSessionRunsState);
  const pendingEventsRef = useRef<WebEvent[]>([]);
  const coalesceFrameRef = useRef<number | null>(null);
  const notificationRef = useRef<ReturnType<typeof createReplyNotifier> | null>(null);

  useEffect(() => {
    if (!workspaceId || !sessionId) return;
    const notifier = createReplyNotifier(workspaceId, sessionId);
    notificationRef.current = notifier;
    return () => {
      notifier.dispose();
      if (notificationRef.current === notifier) notificationRef.current = null;
    };
  }, [workspaceId, sessionId]);

  /** 立即冲刷已合并的高频事件。 */
  const flushPendingEvents = useCallback(() => {
    if (coalesceFrameRef.current !== null) {
      cancelAnimationFrame(coalesceFrameRef.current);
      coalesceFrameRef.current = null;
    }
    const batch = pendingEventsRef.current;
    if (batch.length === 0) return;
    pendingEventsRef.current = [];
    dispatch({ type: "events", events: batch });
  }, []);

  /**
   * 高频 delta 合并到下一帧；终态与控制类事件立即提交。
   *
   * @param event 运行事件
   */
  const enqueueEvent = useCallback((event: WebEvent) => {
    if (COALESCED_EVENT_TYPES.has(event.type)) {
      pendingEventsRef.current.push(event);
      if (coalesceFrameRef.current === null) {
        coalesceFrameRef.current = requestAnimationFrame(() => {
          coalesceFrameRef.current = null;
          const batch = pendingEventsRef.current;
          if (batch.length === 0) return;
          pendingEventsRef.current = [];
          dispatch({ type: "events", events: batch });
        });
      }
      return;
    }
    flushPendingEvents();
    if (event.type === "run.merged") {
      onQueueMerged?.(typeof event.payload.input === "string" ? event.payload.input : "");
    }
    dispatch({ type: "event", event });
  }, [flushPendingEvents, onQueueMerged]);

  useEffect(() => {
    dispatch({ type: "relocalize" });
  }, [locale]);

  useEffect(() => {
    if (!workspaceId || !sessionId) return;
    let cancelled = false;
    void api.runs.interruptionRecovery(workspaceId, sessionId).then(({ run }) => {
      if (!cancelled && run?.restore_input) onInterruptedWithoutReply?.(run.restore_input);
    });
    void api.runs.active().then(({ runs }) => {
      if (cancelled) return;
      const activeRuns = runs.filter((run) => run.workspace_id === workspaceId && run.session_id === sessionId);
      notificationRef.current?.trackActiveRuns(activeRuns);
      dispatch({ type: "attach", sessionId, runs: activeRuns });
    });
    return () => { cancelled = true; };
  }, [workspaceId, sessionId]);

  // 会话级事件流：同一会话的所有标签页与所有轮次共享一条连接，
  // 最后一个已收序号跨重连保留，缺失部分由服务端从落盘日志补发
  const lastSequenceRef = useRef(0);
  useEffect(() => {
    lastSequenceRef.current = 0;
  }, [workspaceId, sessionId]);

  useEffect(() => {
    if (!workspaceId || !sessionId) return;
    let closedByClient = false;
    let reconnectAttempts = 0;
    let reconnectTimer = 0;
    let source: EventSource | null = null;
    const MAX_RECONNECT = 5;

    const failDisconnected = () => {
      const lastSequence = lastSequenceRef.current;
      const summary = text(locale, "Connection interrupted", "连接中断");
      const detail = [
        text(
          locale,
          "The session event stream disconnected after multiple reconnect attempts. You can retry this turn.",
          "会话事件流在多次重连后仍断开。可点击重试本轮。"
        ),
        "",
        text(locale, "Diagnostic context:", "诊断上下文："),
        sessionId ? `session_id=${sessionId}` : null,
        `last_sequence=${lastSequence}`,
        `reconnect_attempts=${reconnectAttempts}`,
        `max_reconnect=${MAX_RECONNECT}`,
        `event_source_path=/api/sessions/${sessionId}/events${lastSequence > 0 ? `?after=${lastSequence}` : ""}`,
        `ready_state_note=${text(
          locale,
          "EventSource closed after retry budget was exhausted.",
          "EventSource 在重试次数耗尽后关闭。"
        )}`
      ]
        .filter((line): line is string => Boolean(line))
        .join("\n");
      flushPendingEvents();
      dispatch({ type: "fail-open", summary, detail });
      onSettled();
    };

    const openSource = () => {
      closedByClient = false;
      const lastSequence = lastSequenceRef.current;
      const query = new URLSearchParams({ workspace_id: workspaceId });
      if (lastSequence > 0) query.set("after", String(lastSequence));
      const next = new EventSource(
        `/api/sessions/${encodeURIComponent(sessionId)}/events?${query.toString()}`
      );
      source = next;

      const handle = (message: MessageEvent<string>) => {
        let event: WebEvent;
        try {
          event = JSON.parse(message.data) as WebEvent;
        } catch (error) {
          event = runFailureEvent(
            "",
            sessionId,
            text(locale, "Invalid run event", "运行事件格式无效"),
            errorDetail(error, message.data)
          );
        }
        if (typeof event.sequence === "number" && event.sequence > lastSequenceRef.current) {
          lastSequenceRef.current = event.sequence;
        }
        reconnectAttempts = 0;
        void notificationRef.current?.accept(event, locale);
        if (event.type === "run.interrupted"
          && event.payload.discard_user_turn === true
          && event.payload.queued !== true) {
          onInterruptedWithoutReply?.(String(event.payload.restore_input ?? ""));
        }
        if (event.type === "stream.lagged") {
          // 服务端摘除了跟不上的观察者；重连并按最后收到的序号补发空洞
          closedByClient = true;
          next.onerror = null;
          next.close();
          reconnectTimer = window.setTimeout(openSource, 100);
          return;
        }
        enqueueEvent(event);
        if (event.type === "workspace.changed") onWorkspaceChanged?.();
        if (event.type === "compaction.finished" && event.payload.applied === true) {
          void Promise.all([
            queryClient.invalidateQueries({ queryKey: ["system-usage"] }),
            queryClient.invalidateQueries({ queryKey: ["timeline", event.session_id || sessionId] })
          ]);
        }
        if (event.type === "session.summary" || event.type === "run.completed") {
          void queryClient.invalidateQueries({ queryKey: ["system-usage"] });
        }
        if (event.type === "context.updated") {
          void queryClient.invalidateQueries({ queryKey: ["system-usage"] });
        }
        if (event.type === "session.renamed") {
          void Promise.all([
            queryClient.invalidateQueries({ queryKey: ["sessions"] }),
            queryClient.invalidateQueries({ queryKey: ["session-tree"] })
          ]);
        }
        if (["run.completed", "run.interrupted", "run.failed"].includes(event.type)) {
          void Promise.all([
            queryClient.invalidateQueries({ queryKey: ["sessions"] }),
            queryClient.invalidateQueries({ queryKey: ["session-tree"] })
          ]);
          // 会话流不因单轮结束而关闭：后续轮次与排队变更仍走同一条连接
          onSettled();
        }
      };
      for (const type of EVENT_TYPES) next.addEventListener(type, handle as EventListener);
      next.onerror = () => {
        if (closedByClient) return;
        if (next.readyState !== EventSource.CLOSED) return;
        reconnectAttempts += 1;
        if (reconnectAttempts > MAX_RECONNECT) {
          failDisconnected();
          return;
        }
        const delay = Math.min(4_000, 300 * 2 ** (reconnectAttempts - 1));
        reconnectTimer = window.setTimeout(openSource, delay);
      };
    };

    openSource();
    return () => {
      window.clearTimeout(reconnectTimer);
      flushPendingEvents();
      if (source) {
        closedByClient = true;
        source.onerror = null;
        source.close();
      }
    };
  }, [enqueueEvent, flushPendingEvents, locale, onInterruptedWithoutReply, onSettled, onWorkspaceChanged, queryClient, sessionId, workspaceId]);

  /**
   * 提交一轮运行；同会话已有运行时由后端持久化排队。
   *
   * @param targetSessionId 目标会话标识
   * @param input 发送给模型的完整输入
   * @param mode 运行权限模式
   * @param selection 可选模型选择
   * @param imageUrls 可选图片列表
   * @param thinkingLevel 可选思考等级
   * @param agentId 可选智能体标识
   * @param displayInput 可选界面显示正文，用于隐藏旁路上下文封装
   * @returns 启动完成后的 Promise
   */
  const start = async (
    targetSessionId: string,
    input: string,
    mode: RunMode,
    selection?: RunModelSelection,
    imageUrls?: string[],
    thinkingLevel?: ThinkingLevel,
    agentId?: string,
    displayInput?: string
  ) => {
    const run = await api.runs.start(targetSessionId, input, mode, selection, imageUrls, thinkingLevel, agentId);
    notificationRef.current?.trackActiveRuns([run]);
    // 记录本次请求的模型，落库前实时消息即可参与模型切换分割线派生
    dispatch({ type: "start", run, sessionId: targetSessionId, userInput: displayInput ?? input, imageUrls, model: selection?.model });
    // 运行创建后立即刷新分支树，先展示新用户轮次，不等待助手回复结束
    void queryClient.invalidateQueries({ queryKey: ["session-turn-tree", targetSessionId] });
  };

  /**
   * 启动当前会话的 Goal 自动续轮。
   *
   * @param targetSessionId 目标会话标识
   * @param mode 当前运行模式
   * @param selection 可选模型选择
   * @param thinkingLevel 可选思考等级
   * @param agentId 可选智能体标识
   * @param displayInput 可选界面显示正文；自动续轮不传入
   * @returns 启动完成后的 Promise
   */
  const startGoal = async (
    targetSessionId: string,
    mode: RunMode,
    selection?: RunModelSelection,
    thinkingLevel?: ThinkingLevel,
    agentId?: string,
    displayInput?: string
  ) => {
    const run = await api.runs.startGoal(targetSessionId, mode, selection, thinkingLevel, agentId);
    dispatch({ type: "start", run, sessionId: targetSessionId, userInput: displayInput ?? "", model: selection?.model });
  };

  /** 使用当前会话模型选择启动一次手动压缩。 */
  const startCompaction = async (
    targetSessionId: string,
    selection?: RunModelSelection
  ) => {
    const run = await api.sessions.compact(targetSessionId, selection);
    dispatch({ type: "start", run, sessionId: targetSessionId, userInput: "" });
  };

  /**
   * 中断指定运行。
   *
   * 1. 本地立即标记终态，避免事件流延迟时界面仍显示思考中
   * 2. 请求服务端停止
   *
   * @param runId 运行标识
   * @returns 停止完成后的 Promise
   */
  const stop = async (runId: string) => {
    dispatch({ type: "stop-local", runId });
    try {
      await api.runs.stop(runId);
    } catch (error) {
      if (workspaceId && sessionId) {
        const { runs } = await api.runs.active();
        dispatch({
          type: "attach",
          sessionId,
          runs: runs.filter((run) => run.workspace_id === workspaceId && run.session_id === sessionId)
        });
      }
      throw error;
    }
  };

  /**
   * 更新排队消息正文和图片附件。
   *
   * @param runId 排队运行标识
   * @param input 新消息正文
   * @param imageUrls 保留的图片附件
   * @returns 更新完成后的 Promise
   */
  const updateQueuedInput = async (runId: string, input: string, imageUrls?: string[]) => {
    const info = await api.runs.updateQueue(runId, { input, image_urls: imageUrls });
    dispatch({
      type: "update-queued",
      runId,
      input: info.input ?? input,
      imageUrls: info.image_urls ?? imageUrls
    });
  };

  /**
   * 移动排队消息。
   *
   * @param runId 排队运行标识
   * @param position 从零开始的目标位置
   * @returns 移动完成后的 Promise
   */
  const moveQueuedRun = async (runId: string, position: number) => {
    await api.runs.updateQueue(runId, { position });
    dispatch({ type: "update-queued", runId, position });
  };

  /**
   * 将排队消息提升到队首，并改为下次模型请求间隙插入。
   *
   * @param runId 排队运行标识
   * @returns 更新完成后的 Promise
   */
  const promoteQueuedRun = async (runId: string) => {
    const info = await api.runs.updateQueue(runId, { position: 0, insert_at: "request" });
    dispatch({
      type: "update-queued",
      runId,
      position: 0,
      insertAt: parseQueueInsertAt(info.insert_at) ?? "request"
    });
  };

  /**
   * 切换排队消息的插入点。
   *
   * @param runId 排队运行标识
   * @param insertAt 目标插入点
   * @returns 更新完成后的 Promise
   */
  const updateQueuedInsertAt = async (runId: string, insertAt: LiveRunState["insertAt"]) => {
    const info = await api.runs.updateQueue(runId, { insert_at: insertAt });
    dispatch({
      type: "update-queued",
      runId,
      insertAt: parseQueueInsertAt(info.insert_at) ?? insertAt
    });
  };

  /**
   * 删除尚未开始的排队消息。
   *
   * @param runId 排队运行标识
   * @returns 删除完成后的 Promise
   */
  const removeQueuedRun = async (runId: string) => {
    await api.runs.stop(runId);
    dispatch({ type: "remove-queued", runId });
  };

  /**
   * 时间线已落盘后丢弃对应的已完成 live run，释放内存并避免重复渲染。
   *
   * @param historyTurns 服务端时间线中的轮次标识与运行状态
   */
  const pruneSettled = useCallback((historyTurns: HistoryTurnKey[]) => {
    if (historyTurns.length === 0) return;
    dispatch({ type: "prune-settled", historyTurns });
  }, []);

  return {
    states: state.runs,
    start,
    startGoal,
    startCompaction,
    stop,
    updateQueuedInput,
    moveQueuedRun,
    promoteQueuedRun,
    updateQueuedInsertAt,
    removeQueuedRun,
    pruneSettled,
    reset: () => {
      flushPendingEvents();
      dispatch({ type: "reset" });
    }
  };
}

/**
 * 将事件解析异常和原始载荷组合为可诊断详情。
 *
 * @param error JSON 解析异常
 * @param payload 原始事件文本
 * @returns 包含异常和载荷的详情文本
 */
function errorDetail(error: unknown, payload: string): string {
  const reason = error instanceof Error ? error.stack || error.message : String(error);
  return `${reason}\n\nEvent payload:\n${payload}`;
}
