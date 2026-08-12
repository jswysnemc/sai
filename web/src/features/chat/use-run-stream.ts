import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useReducer, useRef } from "react";
import type { AppConfig, RunInfo, RunMode, RunModelSelection, ThinkingLevel, WebEvent } from "../../api/contracts";
import { api } from "../../api/client";
import { initialRunState, relocalizeRunError, runEventReducer, type LiveRunState } from "./run-event-reducer";
import { useI18n } from "../i18n/use-i18n";
import { text, type Locale } from "../i18n/locale";
import { notifyReplyComplete } from "../../shared/notify/reply-complete-notify";

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
  "run.failed"
] as const;

type SessionRunsState = { runs: LiveRunState[] };

type SessionRunsAction =
  | { type: "attach"; runs: RunInfo[]; sessionId: string }
  | { type: "start"; run: RunInfo; sessionId: string; userInput: string; imageUrls?: string[]; model?: string }
  | { type: "event"; event: WebEvent }
  | { type: "events"; events: WebEvent[] }
  | { type: "prune-settled"; historyTurnIds: string[] }
  | { type: "update-queued"; runId: string; input?: string; position?: number }
  | { type: "remove-queued"; runId: string }
  | { type: "stop-local"; runId: string }
  | { type: "relocalize" }
  | { type: "reset" };

const initialSessionRunsState: SessionRunsState = { runs: [] };

/** 高频流式事件：合并到同一动画帧再进 reducer，降低 React 提交次数 */
const COALESCED_EVENT_TYPES = new Set<WebEvent["type"]>([
  "message.content.delta",
  "message.reasoning.delta",
  "tool.progress",
  "compaction.delta",
]);

/**
 * 将运行事件归并到会话内对应的实时消息。
 *
 * @param state 当前会话运行集合
 * @param action 运行附加、启动或事件动作
 * @returns 更新后的会话运行集合
 */
export function sessionRunsReducer(state: SessionRunsState, action: SessionRunsAction, locale: Locale = "zh-CN"): SessionRunsState {
  if (action.type === "reset") return initialSessionRunsState;
  if (action.type === "relocalize") {
    return { runs: state.runs.map((run) => ({ ...run, error: relocalizeRunError(run.error, locale) })) };
  }
  if (action.type === "attach") {
    const known = new Set(state.runs.map((run) => run.runId));
    const attached = action.runs
      .filter((run) => !known.has(run.run_id))
      .map((run) => ({
        ...runEventReducer(initialRunState, {
          type: "attach",
          runId: run.run_id,
          sessionId: action.sessionId,
          userInput: run.input ?? "",
          imageUrls: run.image_urls
        }, locale),
        status: run.status === "queued" ? "queued" as const : "waiting_response" as const
      }));
    return { runs: [...state.runs, ...attached] };
  }
  if (action.type === "start") {
    const next = runEventReducer(initialRunState, {
      type: "start",
      runId: action.run.run_id,
      sessionId: action.sessionId,
      userInput: action.userInput,
      imageUrls: action.imageUrls,
      model: action.model
    }, locale);
    return {
      runs: [...state.runs, {
        ...next,
        status: action.run.status === "queued" ? "queued" : next.status
      }]
    };
  }
  if (action.type === "stop-local") {
    return {
      runs: state.runs.map((run) => {
        if (run.runId !== action.runId || run.completed) return run;
        return runEventReducer(run, {
          type: "event",
          event: {
            sequence: 0,
            run_id: action.runId,
            workspace_id: "",
            session_id: run.sessionId ?? "",
            timestamp: new Date().toISOString(),
            type: "run.interrupted",
            payload: {
              discard_user_turn: false,
              restore_input: null,
              detail: "The user stopped this run before it completed."
            }
          }
        }, locale);
      })
    };
  }
  if (action.type === "remove-queued") {
    return { runs: state.runs.filter((run) => run.runId !== action.runId) };
  }
  if (action.type === "update-queued") {
    return updateQueuedRunState(state, action.runId, action.input, action.position);
  }
  if (action.type === "prune-settled") {
    const historyIds = new Set(action.historyTurnIds);
    const runs = state.runs.filter(
      (run) => !(run.completed && run.runId && historyIds.has(run.runId))
    );
    return runs.length === state.runs.length ? state : { runs };
  }
  if (action.type === "events") {
    return applyEventsToSessionRuns(state, action.events, locale);
  }
  return applyEventToSessionRuns(state, action.event, locale);
}

/**
 * 将单条运行事件应用到会话运行集合。
 *
 * @param state 当前会话运行集合
 * @param event 运行事件
 * @param locale 本地化语言
 * @returns 更新后的会话运行集合
 */
function applyEventToSessionRuns(
  state: SessionRunsState,
  event: WebEvent,
  locale: Locale
): SessionRunsState {
  if (event.type === "run.interrupted" && event.payload.discard_user_turn === true) {
    return { runs: state.runs.filter((run) => run.runId !== event.run_id) };
  }
  if (event.type === "run.merged") {
    return { runs: state.runs.filter((run) => run.runId !== event.run_id) };
  }
  if (event.type === "run.queue.updated") {
    return updateQueuedRunState(
      state,
      event.run_id,
      typeof event.payload.input === "string" ? event.payload.input : undefined,
      typeof event.payload.position === "number" ? event.payload.position : undefined
    );
  }
  let changed = false;
  const runs = state.runs.map((run) => {
    if (run.runId !== event.run_id) return run;
    changed = true;
    return runEventReducer(run, { type: "event", event }, locale);
  });
  return changed ? { runs } : state;
}

/**
 * 将同一帧内的多条事件按 run 归并后一次提交，避免逐事件重绘整棵会话树。
 *
 * @param state 当前会话运行集合
 * @param events 待应用事件（保持到达顺序）
 * @param locale 本地化语言
 * @returns 更新后的会话运行集合
 */
export function applyEventsToSessionRuns(
  state: SessionRunsState,
  events: WebEvent[],
  locale: Locale = "zh-CN"
): SessionRunsState {
  if (events.length === 0) return state;
  if (events.length === 1) return applyEventToSessionRuns(state, events[0], locale);

  const byRun = new Map<string, WebEvent[]>();
  const special: WebEvent[] = [];
  for (const event of events) {
    if (
      event.type === "run.interrupted"
      || event.type === "run.merged"
      || event.type === "run.queue.updated"
      || event.type === "run.completed"
      || event.type === "run.failed"
    ) {
      special.push(event);
      continue;
    }
    const batch = byRun.get(event.run_id);
    if (batch) batch.push(event);
    else byRun.set(event.run_id, [event]);
  }

  let next = state;
  if (byRun.size > 0) {
    let changed = false;
    const runs = next.runs.map((run) => {
      const batch = run.runId ? byRun.get(run.runId) : undefined;
      if (!batch?.length) return run;
      changed = true;
      return batch.reduce(
        (current, event) => runEventReducer(current, { type: "event", event }, locale),
        run
      );
    });
    if (changed) next = { runs };
  }
  for (const event of special) {
    next = applyEventToSessionRuns(next, event, locale);
  }
  return next;
}

/**
 * 更新排队运行正文并在会话运行集合中调整位置。
 *
 * @param state 当前会话运行集合
 * @param runId 待更新运行标识
 * @param input 可选新正文
 * @param position 可选目标位置
 * @returns 更新后的会话运行集合
 */
export function updateQueuedRunState(
  state: SessionRunsState,
  runId: string,
  input?: string,
  position?: number
): SessionRunsState {
  const current = state.runs.findIndex((run) => run.runId === runId && run.status === "queued");
  if (current < 0) return state;
  const selected = {
    ...state.runs[current],
    userInput: input ?? state.runs[current].userInput
  };
  if (position === undefined) {
    return { runs: state.runs.map((run, index) => index === current ? selected : run) };
  }

  // 1. 后端位置只针对同一会话的排队项，活动与终态运行保持原相对位置
  const queued = state.runs.filter((run) => run.status === "queued" && run.runId !== runId);
  queued.splice(Math.max(0, Math.min(position, queued.length)), 0, selected);
  let queuedIndex = 0;
  return {
    runs: state.runs.map((run) => run.status === "queued" ? queued[queuedIndex++] : run)
  };
}

/**
 * 管理一个会话中的活动和排队 Agent 运行。
 *
 * @param workspaceId 当前工作区标识
 * @param sessionId 当前会话标识
 * @param onSettled 运行结束回调
 * @param onWorkspaceChanged 工作区文件变化回调
 * @param onInterruptedWithoutReply 无回复中断输入恢复回调
 * @returns 会话运行状态与启动、停止、重置操作
 */
export function useRunStream(
  workspaceId: string | undefined,
  sessionId: string | undefined,
  onSettled: () => void,
  onWorkspaceChanged?: () => void,
  onInterruptedWithoutReply?: (input: string) => void
) {
  const { locale } = useI18n();
  const queryClient = useQueryClient();
  // 预取配置，供答复完成通知读取 notification 开关
  useQuery({ queryKey: ["config"], queryFn: api.config.load });
  const reducer = useCallback(
    (state: SessionRunsState, action: SessionRunsAction) => sessionRunsReducer(state, action, locale),
    [locale]
  );
  const [state, dispatch] = useReducer(reducer, initialSessionRunsState);
  const sourcesRef = useRef(new Map<string, EventSource>());
  const pendingEventsRef = useRef<WebEvent[]>([]);
  const coalesceFrameRef = useRef<number | null>(null);

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
    dispatch({ type: "event", event });
  }, [flushPendingEvents]);

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
      dispatch({
        type: "attach",
        sessionId,
        runs: runs.filter((run) => run.workspace_id === workspaceId && run.session_id === sessionId)
      });
    });
    return () => { cancelled = true; };
  }, [workspaceId, sessionId]);

  const openRunIds = useMemo(
    () => state.runs.filter((run) => run.runId && !run.completed).map((run) => run.runId!),
    [state.runs]
  );
  const openRunKey = openRunIds.join(",");

  useEffect(() => {
    const desired = new Set(openRunIds);
    const reconnectTimers = new Map<string, number>();
    for (const [runId, source] of sourcesRef.current) {
      if (desired.has(runId)) continue;
      source.close();
      sourcesRef.current.delete(runId);
    }
    for (const runId of openRunIds) {
      if (sourcesRef.current.has(runId)) continue;
      // 断连自动重连：带 after=sequence 续订，避免丢事件
      let lastSequence = 0;
      let reconnectAttempts = 0;
      let closedByClient = false;
      const MAX_RECONNECT = 5;

      const failDisconnected = () => {
        const summary = text(locale, "Connection interrupted", "连接中断");
        const detail = [
          text(
            locale,
            "The run event stream disconnected after multiple reconnect attempts. You can retry this turn.",
            "运行事件流在多次重连后仍断开。可点击重试本轮。"
          ),
          "",
          text(locale, "Diagnostic context:", "诊断上下文："),
          `run_id=${runId}`,
          sessionId ? `session_id=${sessionId}` : null,
          `last_sequence=${lastSequence}`,
          `reconnect_attempts=${reconnectAttempts}`,
          `max_reconnect=${MAX_RECONNECT}`,
          `event_source_path=/api/runs/${runId}/events${lastSequence > 0 ? `?after=${lastSequence}` : ""}`,
          `ready_state_note=${text(
            locale,
            "EventSource closed after retry budget was exhausted.",
            "EventSource 在重试次数耗尽后关闭。"
          )}`
        ]
          .filter((line): line is string => Boolean(line))
          .join("\n");
        enqueueEvent(runFailureEvent(runId, sessionId, summary, detail));
        sourcesRef.current.delete(runId);
        onSettled();
      };

      const openSource = () => {
        if (closedByClient) return;
        const query = lastSequence > 0 ? `?after=${lastSequence}` : "";
        const source = new EventSource(`/api/runs/${runId}/events${query}`);
        sourcesRef.current.set(runId, source);

        const handle = (message: MessageEvent<string>) => {
          let event: WebEvent;
          try {
            event = JSON.parse(message.data) as WebEvent;
          } catch (error) {
            event = runFailureEvent(
              runId,
              sessionId,
              text(locale, "Invalid run event", "运行事件格式无效"),
              errorDetail(error, message.data)
            );
          }
          if (typeof event.sequence === "number" && event.sequence > lastSequence) {
            lastSequence = event.sequence;
          }
          reconnectAttempts = 0;
          if (event.type === "run.interrupted"
            && event.payload.discard_user_turn === true
            && event.payload.queued !== true) {
            onInterruptedWithoutReply?.(String(event.payload.restore_input ?? ""));
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
          if (event.type === "run.merged") {
            closedByClient = true;
            source.onerror = null;
            source.close();
            sourcesRef.current.delete(runId);
            return;
          }
          if (["run.completed", "run.interrupted", "run.failed"].includes(event.type)) {
            void Promise.all([
              queryClient.invalidateQueries({ queryKey: ["sessions"] }),
              queryClient.invalidateQueries({ queryKey: ["session-tree"] })
            ]);
            const response = queryClient.getQueryData(["config"]) as { config?: AppConfig } | undefined;
            const body =
              event.type === "run.interrupted"
                ? text(locale, "Reply interrupted", "答复已中断")
                : event.type === "run.failed"
                  ? text(locale, "Reply failed", "答复失败")
                  : text(locale, "Reply complete", "答复已完成");
            notifyReplyComplete(response?.config?.notification, text(locale, "Sai", "Sai"), body);
            closedByClient = true;
            source.onerror = null;
            source.close();
            sourcesRef.current.delete(runId);
            onSettled();
          }
        };
        for (const type of EVENT_TYPES) source.addEventListener(type, handle as EventListener);
        source.onerror = () => {
          if (closedByClient) return;
          if (source.readyState !== EventSource.CLOSED) return;
          sourcesRef.current.delete(runId);
          reconnectAttempts += 1;
          if (reconnectAttempts > MAX_RECONNECT) {
            failDisconnected();
            return;
          }
          const delay = Math.min(4_000, 300 * 2 ** (reconnectAttempts - 1));
          const timer = window.setTimeout(() => {
            reconnectTimers.delete(runId);
            openSource();
          }, delay);
          reconnectTimers.set(runId, timer);
        };
      };

      openSource();
    }
    return () => {
      for (const timer of reconnectTimers.values()) window.clearTimeout(timer);
      flushPendingEvents();
    };
  }, [enqueueEvent, flushPendingEvents, locale, openRunIds, openRunKey, onInterruptedWithoutReply, onSettled, onWorkspaceChanged, queryClient, sessionId]);

  useEffect(() => () => {
    flushPendingEvents();
    for (const source of sourcesRef.current.values()) source.close();
    sourcesRef.current.clear();
  }, [flushPendingEvents]);

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
    // 1. 先本地结束运行态，保证停止按钮立即生效
    dispatch({ type: "stop-local", runId });
    // 2. 再请求服务端中断；即使服务端已结束也保持本地终态
    try {
      await api.runs.stop(runId);
    } catch {
      // 服务端停止失败时仍保留本地终态，避免界面卡在思考中
    }
  };

  /**
   * 更新排队消息正文。
   *
   * @param runId 排队运行标识
   * @param input 新消息正文
   * @returns 更新完成后的 Promise
   */
  const updateQueuedInput = async (runId: string, input: string) => {
    const info = await api.runs.updateQueue(runId, { input });
    dispatch({ type: "update-queued", runId, input: info.input ?? input });
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
   * 将排队消息提升到当前会话队首。
   *
   * @param runId 排队运行标识
   * @returns 移动完成后的 Promise
   */
  const promoteQueuedRun = (runId: string) => moveQueuedRun(runId, 0);

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
   * @param historyTurnIds 服务端时间线中的轮次标识
   */
  const pruneSettled = useCallback((historyTurnIds: string[]) => {
    if (historyTurnIds.length === 0) return;
    dispatch({ type: "prune-settled", historyTurnIds });
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
    removeQueuedRun,
    pruneSettled,
    reset: () => {
      flushPendingEvents();
      dispatch({ type: "reset" });
    }
  };
}

/**
 * 构造仅供前端状态归并使用的运行失败事件。
 *
 * @param runId 运行标识
 * @param sessionId 会话标识
 * @param message 面向用户的错误摘要
 * @param detail 原始错误详情
 * @returns 与服务端终态事件结构一致的失败事件
 */
function runFailureEvent(runId: string, sessionId: string | undefined, message: string, detail: string): WebEvent {
  return {
    sequence: 0,
    run_id: runId,
    workspace_id: "",
    session_id: sessionId ?? "",
    timestamp: new Date().toISOString(),
    type: "run.failed",
    payload: { message, detail }
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
