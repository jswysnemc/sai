import type { PendingQuestion, PermissionDecision, PermissionRequest, QuestionResponse, WebEvent } from "../../api/contracts";
import { text, type Locale } from "../i18n/locale";

export type ToolLifecycle = {
  id: string;
  name: string;
  argumentsPreview: string;
  arguments: string;
  progress: string;
  output: string;
  status: "preparing" | "running" | "completed" | "failed";
};

export type LiveMessagePart =
  | { id: string; type: "reasoning"; source: string; startedAt: string; endedAt?: string }
  | { id: string; type: "text"; source: string }
  | { id: string; type: "automatic_input"; kind: string; source: string }
  | { id: string; type: "tool"; tool: ToolLifecycle }
  | { id: string; type: "permission"; request: PermissionRequest; decision?: PermissionDecision }
  | { id: string; type: "question"; pending: PendingQuestion; response?: QuestionResponse }
  | { id: string; type: "compaction"; status: "running" | "completed"; turnCount: number; model?: string; applied?: boolean; summary?: string; error?: RunErrorDetail };

export type RunErrorDetail = {
  message: string;
  detail: string;
};

export type LiveRunState = {
  runId: string | null;
  sessionId: string | null;
  status: "idle" | "queued" | "waiting_response" | "waiting_external" | "waiting_permission" | "waiting_question" | "thinking" | "working" | "compacting";
  userInput: string;
  imageUrls: string[];
  content: string;
  reasoning: string;
  tools: ToolLifecycle[];
  parts: LiveMessagePart[];
  error: string | null;
  errorDetail: string | null;
  completed: boolean;
};

export type RunAction =
  | { type: "start"; runId: string; sessionId: string; userInput: string; imageUrls?: string[] }
  | { type: "attach"; runId: string; sessionId: string; userInput: string; imageUrls?: string[] }
  | { type: "event"; event: WebEvent }
  | { type: "reset" };

export const initialRunState: LiveRunState = {
  runId: null,
  sessionId: null,
  status: "idle",
  userInput: "",
  imageUrls: [],
  content: "",
  reasoning: "",
  tools: [],
  parts: [],
  error: null,
  errorDetail: null,
  completed: false
};

/**
 * 将运行状态中的内置错误文案切换到指定语言。
 *
 * @param message 当前错误文案
 * @param locale 目标界面语言
 * @returns 本地化后的内置文案；服务端原始错误保持不变
 */
export function relocalizeRunError(message: string | null, locale: Locale): string | null {
  if (message === "Run failed" || message === "运行失败") return text(locale, "Run failed", "运行失败");
  if (message === "The response was interrupted; generated content was preserved" || message === "响应已中断，已保留生成内容") {
    return text(locale, "The response was interrupted; generated content was preserved", "响应已中断，已保留生成内容");
  }
  if (message === "The run was interrupted" || message === "运行已中断") {
    return text(locale, "The run was interrupted", "运行已中断");
  }
  return message;
}

/** 将后端事件归并为单轮聊天与工具生命周期状态。 */
export function runEventReducer(state: LiveRunState, action: RunAction, locale: Locale = "zh-CN"): LiveRunState {
  if (action.type === "reset") return initialRunState;
  if (action.type === "start" || action.type === "attach") {
    return { ...initialRunState, runId: action.runId, sessionId: action.sessionId, userInput: action.userInput, imageUrls: action.imageUrls ?? [], status: "waiting_response" };
  }
  const { event } = action;
  const payload = event.payload;
  switch (event.type) {
    case "status.changed":
      return { ...state, status: String(payload.status) as LiveRunState["status"] };
    case "run.queued":
      return { ...state, status: "queued" };
    case "run.dequeued":
    case "run.started":
      return { ...state, status: "waiting_response" };
    case "message.automatic.input":
      return {
        ...closeActiveReasoning(state, event.timestamp),
        status: "waiting_response",
        parts: [...state.parts, {
          id: `automatic-input-${event.sequence}`,
          type: "automatic_input",
          kind: String(payload.kind ?? "automatic"),
          source: String(payload.content ?? "")
        }]
      };
    case "message.content.delta":
      return appendTextPart(closeActiveReasoning(state, event.timestamp), event.sequence, String(payload.text ?? ""));
    case "message.reasoning.delta":
      return appendReasoningPart(state, event.sequence, event.timestamp, String(payload.text ?? ""));
    case "tool.call.preparing":
      return upsertTool(closeActiveReasoning(state, event.timestamp), String(payload.tool_id), {
        name: String(payload.name ?? "tool"),
        argumentsPreview: String(payload.arguments_preview ?? ""),
        status: "preparing"
      });
    case "tool.call.started":
      return upsertTool(closeActiveReasoning(state, event.timestamp), String(payload.tool_id), {
        name: String(payload.name ?? "tool"),
        arguments: String(payload.arguments ?? ""),
        argumentsPreview: String(payload.arguments ?? ""),
        status: "running"
      });
    case "tool.progress":
      return upsertTool(closeActiveReasoning(state, event.timestamp), String(payload.tool_id), {
        name: String(payload.name ?? "tool"),
        progress: String(payload.message ?? ""),
        status: "running"
      });
    case "tool.result":
      return upsertTool(closeActiveReasoning(state, event.timestamp), String(payload.tool_id), {
        name: String(payload.name ?? "tool"),
        output: String(payload.output ?? ""),
        status: payload.ok === false ? "failed" : "completed"
      });
    case "permission.requested":
      return upsertPermissionPart({
        ...closeActiveReasoning(state, event.timestamp),
        status: "waiting_permission"
      }, payload as unknown as PermissionRequest);
    case "permission.resolved":
      return resolvePermissionPart(
        { ...state, status: "working" },
        String(payload.request_id),
        payload.decision as unknown as PermissionDecision
      );
    case "question.requested":
      return upsertQuestionPart({
        ...closeActiveReasoning(state, event.timestamp),
        status: "waiting_question"
      }, payload as unknown as PendingQuestion);
    case "question.resolved":
      return resolveQuestionPart(
        { ...state, status: "working" },
        String(payload.request_id),
        payload.response as unknown as QuestionResponse
      );
    case "compaction.started":
      return {
        ...closeActiveReasoning(state, event.timestamp),
        parts: [...state.parts, {
          id: `compaction-${event.sequence}`,
          type: "compaction",
          status: "running",
          turnCount: Number(payload.turn_count ?? 0),
          model: typeof payload.model === "string" ? payload.model : undefined
        }]
      };
    case "compaction.delta":
      return appendCompactionDelta(state, String(payload.text ?? ""));
    case "compaction.finished":
      return finishCompaction(
        state,
        Boolean(payload.applied),
        typeof payload.summary === "string" ? payload.summary : undefined,
        parseRunError(payload.error)
      );
    case "run.failed":
      return {
        ...closeActiveReasoning(state, event.timestamp),
        error: String(payload.message ?? text(locale, "Run failed", "运行失败")),
        errorDetail: typeof payload.detail === "string" ? payload.detail : null,
        status: "idle",
        completed: true
      };
    case "run.interrupted":
      return {
        ...closeActiveReasoning(state, event.timestamp),
        error: state.content
          ? text(locale, "The response was interrupted; generated content was preserved", "响应已中断，已保留生成内容")
          : text(locale, "The run was interrupted", "运行已中断"),
        status: "idle",
        completed: true
      };
    case "run.completed":
      return { ...closeActiveReasoning(state, event.timestamp), status: "idle", completed: true };
    default:
      return state;
  }
}

function resolvePermissionPart(state: LiveRunState, requestId: string, decision: PermissionDecision): LiveRunState {
  return {
    ...state,
    parts: state.parts.map((part) => part.type === "permission" && part.request.id === requestId
      ? { ...part, decision }
      : part)
  };
}

function resolveQuestionPart(state: LiveRunState, requestId: string, response: QuestionResponse): LiveRunState {
  return {
    ...state,
    parts: state.parts.map((part) => part.type === "question" && part.pending.id === requestId
      ? { ...part, response }
      : part)
  };
}

function upsertPermissionPart(state: LiveRunState, request: PermissionRequest): LiveRunState {
  const id = `permission-${request.id}`;
  const existing = state.parts.findIndex((part) => part.type === "permission" && part.request.id === request.id);
  if (existing === -1) return { ...state, parts: [...state.parts, { id, type: "permission", request }] };
  return {
    ...state,
    parts: state.parts.map((part, index) => index === existing ? { id, type: "permission" as const, request } : part)
  };
}

function upsertQuestionPart(state: LiveRunState, pending: PendingQuestion): LiveRunState {
  const id = `question-${pending.id}`;
  const existing = state.parts.findIndex((part) => part.type === "question" && part.pending.id === pending.id);
  if (existing === -1) return { ...state, parts: [...state.parts, { id, type: "question", pending }] };
  return {
    ...state,
    parts: state.parts.map((part, index) => index === existing ? { id, type: "question" as const, pending } : part)
  };
}

function finishCompaction(state: LiveRunState, applied: boolean, summary?: string, error?: RunErrorDetail): LiveRunState {
  for (let index = state.parts.length - 1; index >= 0; index -= 1) {
    const part = state.parts[index];
    if (part.type !== "compaction" || part.status !== "running") continue;
    return {
      ...state,
      parts: state.parts.map((item, itemIndex) => itemIndex === index && item.type === "compaction"
        ? {
            ...item,
            status: "completed",
            applied,
            summary: applied && summary?.trim() ? summary.trim() : item.summary,
            error
          }
        : item)
    };
  }
  return state;
}

function appendCompactionDelta(state: LiveRunState, text: string): LiveRunState {
  if (!text) return state;
  for (let index = state.parts.length - 1; index >= 0; index -= 1) {
    const part = state.parts[index];
    if (part.type !== "compaction" || part.status !== "running") continue;
    return {
      ...state,
      parts: state.parts.map((item, itemIndex) => itemIndex === index && item.type === "compaction"
        ? { ...item, summary: (item.summary ?? "") + text }
        : item)
    };
  }
  return state;
}

function parseRunError(value: unknown): RunErrorDetail | undefined {
  if (!value || typeof value !== "object") return undefined;
  const candidate = value as Record<string, unknown>;
  if (typeof candidate.message !== "string" || typeof candidate.detail !== "string") return undefined;
  return { message: candidate.message, detail: candidate.detail };
}

function upsertTool(state: LiveRunState, id: string, patch: Partial<ToolLifecycle>): LiveRunState {
  const index = state.tools.findIndex((tool) => tool.id === id);
  const base: ToolLifecycle = {
    id,
    name: "tool",
    argumentsPreview: "",
    arguments: "",
    progress: "",
    output: "",
    status: "preparing"
  };
  if (index === -1) {
    const tool = { ...base, ...patch };
    return { ...state, tools: [...state.tools, tool], parts: [...state.parts, { id: `tool-${id}`, type: "tool", tool }] };
  }
  const existing = state.tools[index];
  if (patch.name && existing.name !== "tool" && patch.name !== existing.name) {
    const forkedId = `${id}-${patch.name}`;
    return upsertTool(state, forkedId, patch);
  }
  const tools = state.tools.map((tool, toolIndex) => toolIndex === index ? { ...tool, ...patch } : tool);
  return {
    ...state,
    tools,
    parts: state.parts.map((part) => part.type === "tool" && part.tool.id === id ? { ...part, tool: tools[index] } : part)
  };
}

function appendTextPart(state: LiveRunState, sequence: number, text: string): LiveRunState {
  const last = state.parts.at(-1);
  const parts = last?.type === "text"
    ? state.parts.map((part, index) => index === state.parts.length - 1 && part.type === "text" ? { ...part, source: part.source + text } : part)
    : [...state.parts, { id: `text-${sequence}`, type: "text" as const, source: text }];
  return { ...state, content: state.content + text, parts };
}

function appendReasoningPart(state: LiveRunState, sequence: number, timestamp: string, text: string): LiveRunState {
  const last = state.parts.at(-1);
  const parts = last?.type === "reasoning" && !last.endedAt
    ? state.parts.map((part, index) => index === state.parts.length - 1 && part.type === "reasoning" ? { ...part, source: part.source + text } : part)
    : [...state.parts, { id: `reasoning-${sequence}`, type: "reasoning" as const, source: text, startedAt: timestamp }];
  return { ...state, reasoning: state.reasoning + text, parts };
}

function closeActiveReasoning(state: LiveRunState, timestamp: string): LiveRunState {
  const last = state.parts.at(-1);
  if (last?.type !== "reasoning" || last.endedAt) return state;
  return {
    ...state,
    parts: state.parts.map((part, index) => index === state.parts.length - 1 && part.type === "reasoning" ? { ...part, endedAt: timestamp } : part)
  };
}
