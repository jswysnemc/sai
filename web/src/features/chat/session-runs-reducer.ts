import type { RunInfo, WebEvent } from "../../api/contracts";
import { runFailureEvent } from "./run-failure-event";
import { initialRunState, parseQueueInsertAt, relocalizeRunError, runEventReducer, type LiveRunState } from "./run-event-reducer";
import type { Locale } from "../i18n/locale";

export type SessionRunsState = { runs: LiveRunState[] };

/** prune-settled 用的历史轮次摘要：标识与是否仍在运行。 */
export type HistoryTurnKey = {
  turnId: string;
  running: boolean;
};

export type SessionRunsAction =
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

export const initialSessionRunsState: SessionRunsState = { runs: [] };



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

