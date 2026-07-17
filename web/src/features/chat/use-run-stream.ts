import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useReducer, useRef } from "react";
import type { RunInfo, RunMode, RunModelSelection, ThinkingLevel, WebEvent } from "../../api/contracts";
import { api } from "../../api/client";
import { initialRunState, runEventReducer, type LiveRunState } from "./run-event-reducer";

const EVENT_TYPES = [
  "run.queued",
  "run.dequeued",
  "run.started",
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
  | { type: "reset" };

const initialSessionRunsState: SessionRunsState = { runs: [] };

/**
 * 将运行事件归并到会话内对应的实时消息。
 *
 * @param state 当前会话运行集合
 * @param action 运行附加、启动或事件动作
 * @returns 更新后的会话运行集合
 */
export function sessionRunsReducer(state: SessionRunsState, action: SessionRunsAction): SessionRunsState {
  if (action.type === "reset") return initialSessionRunsState;
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
        }),
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
    });
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
      ? runEventReducer(run, { type: "event", event: action.event })
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
  const queryClient = useQueryClient();
  const [state, dispatch] = useReducer(sessionRunsReducer, initialSessionRunsState);
  const sourcesRef = useRef(new Map<string, EventSource>());

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
    for (const [runId, source] of sourcesRef.current) {
      if (desired.has(runId)) continue;
      source.close();
      sourcesRef.current.delete(runId);
    }
    for (const runId of openRunIds) {
      if (sourcesRef.current.has(runId)) continue;
      const source = new EventSource(`/api/runs/${runId}/events`);
      const handle = (message: MessageEvent<string>) => {
        const event = JSON.parse(message.data) as WebEvent;
        if (event.type === "run.interrupted" && event.payload.discard_user_turn === true) {
          onInterruptedWithoutReply?.(String(event.payload.restore_input ?? ""));
        }
        dispatch({ type: "event", event });
        if (event.type === "workspace.changed") onWorkspaceChanged?.();
        // 压缩完成后立刻刷新顶栏上下文占用与会话时间线
        if (event.type === "compaction.finished" && event.payload.applied === true) {
          void Promise.all([
            queryClient.invalidateQueries({ queryKey: ["system-usage"] }),
            queryClient.invalidateQueries({ queryKey: ["timeline", event.session_id || sessionId] })
          ]);
        }
        // 轮次结束时 usage 会更新 prompt_tokens，同步刷新顶栏
        if (event.type === "session.summary" || event.type === "run.completed") {
          void queryClient.invalidateQueries({ queryKey: ["system-usage"] });
        }
        if (["run.completed", "run.interrupted", "run.failed"].includes(event.type)) {
          source.close();
          sourcesRef.current.delete(runId);
          onSettled();
        }
      };
      for (const type of EVENT_TYPES) source.addEventListener(type, handle as EventListener);
      sourcesRef.current.set(runId, source);
    }
  }, [openRunKey, onInterruptedWithoutReply, onSettled, onWorkspaceChanged, queryClient, sessionId]);

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

  return { states: state.runs, start, startCompaction, stop, reset: () => dispatch({ type: "reset" }) };
}
