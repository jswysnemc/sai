import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useReducer, useRef } from "react";
import type { AppConfig, RunInfo, RunMode, RunModelSelection, ThinkingLevel, WebEvent } from "../../api/contracts";
import { api } from "../../api/client";
import { initialRunState, parseQueueInsertAt, relocalizeRunError, runEventReducer, type LiveRunState } from "./run-event-reducer";
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
  "run.failed",
  "stream.lagged"
] as const;

type SessionRunsState = { runs: LiveRunState[] };

/** prune-settled 用的历史轮次摘要：标识与是否仍在运行。 */
export type HistoryTurnKey = {
  turnId: string;
  running: boolean;
};

type SessionRunsAction =
  | { type: "attach"; runs: RunInfo[]; sessionId: string }
  | { type: "start"; run: RunInfo; sessionId: string; userInput: string; imageUrls?: string[]; model?: string }
  | { type: "event"; event: WebEvent }
  | { type: "events"; events: WebEvent[] }
  | { type: "prune-settled"; historyTurns: HistoryTurnKey[] }
  | { type: "update-queued"; runId: string; input?: string; position?: number; insertAt?: LiveRunState["insertAt"]; imageUrls?: string[] }
  | { type: "remove-queued"; runId: string }
  | { type: "stop-local"; runId: string }
  | { type: "fail-open"; summary: string; detail: string }
  | { type: "relocalize" }
  | { type: "reset" };

/**
 * 能够独立重建一轮运行入口的事件类型。
 *
 * 这些事件由服务端在同一会话的事件流上广播，并携带本轮用户输入，
 * 因此后加入的标签页可以只凭事件流补出用户气泡。
 */
const RUN_ENTRY_EVENT_TYPES = new Set<WebEvent["type"]>([
  "run.queued",
  "run.dequeued",
  "run.started",
]);

/** 由事件流创建一轮运行时的初始状态。 */
function statusForRunEntryEvent(type: WebEvent["type"]): LiveRunState["status"] {
  return type === "run.queued" ? "queued" : "waiting_response";
}

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
          imageUrls: run.image_urls,
          insertAt: parseQueueInsertAt(run.insert_at)
        }, locale),
        status: run.status === "queued" ? "queued" as const : "waiting_response" as const
      }));
    return { runs: [...state.runs, ...attached] };
  }
  if (action.type === "start") {
    // 服务端随后会广播同一轮的 run.started。两条路径可能任意先后到达，
    // 因此这里按 run_id 幂等处理：已存在时只补齐本地显示正文与模型，
    // 既不会出现两条用户气泡，也不会让服务端的完整输入盖掉显示正文
    const existing = state.runs.findIndex((run) => run.runId === action.run.run_id);
    if (existing >= 0) {
      return {
        runs: state.runs.map((run, index) => (index === existing
          ? {
            ...run,
            userInput: action.userInput,
            model: run.model ?? action.model ?? null
          }
          : run))
      };
    }
    const next = runEventReducer(initialRunState, {
      type: "start",
      runId: action.run.run_id,
      sessionId: action.sessionId,
      userInput: action.userInput,
      imageUrls: action.imageUrls,
      model: action.model,
      insertAt: parseQueueInsertAt(action.run.insert_at)
    }, locale);
    return {
      runs: [...state.runs, {
        ...next,
        status: action.run.status === "queued" ? "queued" : next.status
      }]
    };
  }
  if (action.type === "fail-open") {
    // 会话事件流断开时把所有未结束的运行一起置为失败，避免界面永久停在思考中
    let changed = false;
    const runs = state.runs.map((run) => {
      if (run.completed || !run.runId) return run;
      changed = true;
      return runEventReducer(run, {
        type: "event",
        event: runFailureEvent(run.runId, run.sessionId ?? undefined, action.summary, action.detail)
      }, locale);
    });
    return changed ? { runs } : state;
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
    return updateQueuedRunState(state, action.runId, action.input, action.position, action.insertAt, action.imageUrls);
  }
  if (action.type === "prune-settled") {
    // 历史轮次已落盘的运行不再重复渲染；重放截断丢失终态事件、但历史里
    // 已是终态的运行同样清掉——否则它的用户气泡会永久堆在会话底部
    const history = new Map(action.historyTurns.map((turn) => [turn.turnId, turn]));
    const runs = state.runs.filter((run) => {
      if (!run.runId) return true;
      const turn = history.get(run.runId);
      if (!turn) return true;
      if (run.completed) return false;
      return turn.running;
    });
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
  // 轮次入口事件对未知 run_id 也要生效，新标签页正是靠它补出用户气泡
  if (RUN_ENTRY_EVENT_TYPES.has(event.type)) {
    return upsertRunFromEvent(state, event, locale);
  }
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
      typeof event.payload.position === "number" ? event.payload.position : undefined,
      parseQueueInsertAt(event.payload.insert_at),
      Array.isArray(event.payload.image_urls) ? event.payload.image_urls as string[] : undefined
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
 * 按 run_id 幂等地创建或更新一轮运行。
 *
 * 同一会话的多个标签页共享一条会话事件流，服务端广播的 run.started /
 * run.queued / run.dequeued 携带本轮输入。已存在时只应用事件本身，
 * 不存在时补建运行入口，保证重复投递不会产生两条用户气泡。
 *
 * @param state 当前会话运行集合
 * @param event 轮次入口事件
 * @param locale 本地化语言
 * @returns 更新后的会话运行集合
 */
export function upsertRunFromEvent(
  state: SessionRunsState,
  event: WebEvent,
  locale: Locale = "zh-CN"
): SessionRunsState {
  const existing = state.runs.findIndex((run) => run.runId === event.run_id);
  if (existing >= 0) {
    return {
      runs: state.runs.map((run, index) => (
        index === existing ? runEventReducer(run, { type: "event", event }, locale) : run
      ))
    };
  }
  const created = runEventReducer(initialRunState, {
    type: "attach",
    runId: event.run_id,
    sessionId: event.session_id,
    userInput: typeof event.payload.input === "string" ? event.payload.input : "",
    imageUrls: Array.isArray(event.payload.image_urls)
      ? event.payload.image_urls as string[]
      : [],
    insertAt: parseQueueInsertAt(event.payload.insert_at)
  }, locale);
  return {
    runs: [...state.runs, { ...created, status: statusForRunEntryEvent(event.type) }]
  };
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
  const entries: WebEvent[] = [];
  for (const event of events) {
    if (RUN_ENTRY_EVENT_TYPES.has(event.type)) {
      entries.push(event);
      continue;
    }
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
  // 1. 轮次入口事件必须早于增量：回放历史时增量才能落到已建好的运行上
  for (const event of entries) {
    next = upsertRunFromEvent(next, event, locale);
  }
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
 * 更新排队运行正文、附件并在会话运行集合中调整位置。
 *
 * @param state 当前会话运行集合
 * @param runId 待更新运行标识
 * @param input 可选新正文
 * @param position 可选目标位置
 * @param insertAt 可选插入点
 * @param imageUrls 可选图片附件
 * @returns 更新后的会话运行集合
 */
export function updateQueuedRunState(
  state: SessionRunsState,
  runId: string,
  input?: string,
  position?: number,
  insertAt?: LiveRunState["insertAt"],
  imageUrls?: string[]
): SessionRunsState {
  const current = state.runs.findIndex((run) => run.runId === runId && run.status === "queued");
  if (current < 0) return state;
  const selected = {
    ...state.runs[current],
    userInput: input ?? state.runs[current].userInput,
    insertAt: insertAt ?? state.runs[current].insertAt,
    imageUrls: imageUrls ?? state.runs[current].imageUrls
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
  // 预取配置，供答复完成通知读取 notification 开关
  useQuery({ queryKey: ["config"], queryFn: api.config.load });
  const reducer = useCallback(
    (state: SessionRunsState, action: SessionRunsAction) => sessionRunsReducer(state, action, locale),
    [locale]
  );
  const [state, dispatch] = useReducer(reducer, initialSessionRunsState);
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
      dispatch({
        type: "attach",
        sessionId,
        runs: runs.filter((run) => run.workspace_id === workspaceId && run.session_id === sessionId)
      });
    });
    return () => { cancelled = true; };
  }, [workspaceId, sessionId]);

  // 会话级事件流：同一会话的所有标签页与所有轮次共享一条连接，
  // 最后一个已收序号跨重连保留，缺失部分由服务端从落盘日志补发
  const lastSequenceRef = useRef(0);
  useEffect(() => {
    lastSequenceRef.current = 0;
  }, [sessionId]);

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
          const response = queryClient.getQueryData(["config"]) as { config?: AppConfig } | undefined;
          const body =
            event.type === "run.interrupted"
              ? text(locale, "Reply interrupted", "答复已中断")
              : event.type === "run.failed"
                ? text(locale, "Reply failed", "答复失败")
                : text(locale, "Reply complete", "答复已完成");
          notifyReplyComplete(response?.config?.notification, text(locale, "Sai", "Sai"), body);
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
