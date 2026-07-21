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
  "run.dequeued",
  "run.started",
  "message.automatic.input",
  "status.changed",
  "message.content.delta",
  "message.reasoning.delta",
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
  "compaction.started",
  "compaction.delta",
  "compaction.finished",
  "loaded_tools.changed",
  "session.summary",
  "run.completed",
  "run.interrupted",
  "run.failed"
] as const;

type SessionRunsState = { runs: LiveRunState[] };

type SessionRunsAction =
  | { type: "attach"; runs: RunInfo[]; sessionId: string }
  | { type: "start"; run: RunInfo; sessionId: string; userInput: string; imageUrls?: string[] }
  | { type: "event"; event: WebEvent }
  | { type: "relocalize" }
  | { type: "reset" };

const initialSessionRunsState: SessionRunsState = { runs: [] };

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
      imageUrls: action.imageUrls
    }, locale);
    return {
      runs: [...state.runs, {
        ...next,
        status: action.run.status === "queued" ? "queued" : next.status
      }]
    };
  }
  if (action.event.type === "run.interrupted" && action.event.payload.discard_user_turn === true) {
    return { runs: state.runs.filter((run) => run.runId !== action.event.run_id) };
  }
  return {
    runs: state.runs.map((run) => run.runId === action.event.run_id
      ? runEventReducer(run, { type: "event", event: action.event }, locale)
      : run)
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
        dispatch({
          type: "event",
          event: runFailureEvent(
            runId,
            sessionId,
            text(locale, "Connection interrupted", "连接中断"),
            text(
              locale,
              "The run event stream disconnected after multiple reconnect attempts. You can retry this turn.",
              "运行事件流在多次重连后仍断开。可点击重试本轮。"
            )
          )
        });
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
          if (event.type === "run.interrupted" && event.payload.discard_user_turn === true) {
            onInterruptedWithoutReply?.(String(event.payload.restore_input ?? ""));
          }
          dispatch({ type: "event", event });
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
          if (["run.completed", "run.interrupted", "run.failed"].includes(event.type)) {
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
    };
  }, [locale, openRunIds, openRunKey, onInterruptedWithoutReply, onSettled, onWorkspaceChanged, queryClient, sessionId]);

  useEffect(() => () => {
    for (const source of sourcesRef.current.values()) source.close();
    sourcesRef.current.clear();
  }, []);

  /**
   * 提交一轮运行；同会话已有运行时由后端持久化排队。
   */
  const start = async (
    targetSessionId: string,
    input: string,
    mode: RunMode,
    selection?: RunModelSelection,
    imageUrls?: string[],
    thinkingLevel?: ThinkingLevel,
    agentId?: string
  ) => {
    const run = await api.runs.start(targetSessionId, input, mode, selection, imageUrls, thinkingLevel, agentId);
    dispatch({ type: "start", run, sessionId: targetSessionId, userInput: input, imageUrls });
  };

  /**
   * 启动当前会话的 Goal 自动续轮。
   *
   * @param targetSessionId 目标会话标识
   * @param mode 当前运行模式
   * @param selection 可选模型选择
   * @param thinkingLevel 可选思考等级
   * @param agentId 可选智能体标识
   * @returns 启动完成后的 Promise
   */
  const startGoal = async (
    targetSessionId: string,
    mode: RunMode,
    selection?: RunModelSelection,
    thinkingLevel?: ThinkingLevel,
    agentId?: string
  ) => {
    const run = await api.runs.startGoal(targetSessionId, mode, selection, thinkingLevel, agentId);
    dispatch({ type: "start", run, sessionId: targetSessionId, userInput: "" });
  };

  /** 使用当前会话模型选择启动一次手动压缩。 */
  const startCompaction = async (
    targetSessionId: string,
    selection?: RunModelSelection
  ) => {
    const run = await api.sessions.compact(targetSessionId, selection);
    dispatch({ type: "start", run, sessionId: targetSessionId, userInput: "" });
  };

  /** 中断指定运行。 */
  const stop = async (runId: string) => {
    await api.runs.stop(runId);
  };

  return { states: state.runs, start, startGoal, startCompaction, stop, reset: () => dispatch({ type: "reset" }) };
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
