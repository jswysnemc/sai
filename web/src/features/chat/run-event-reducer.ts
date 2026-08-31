import type { PendingQuestion, PermissionDecision, PermissionRequest, QueueInsertAt, QuestionResponse, SshSecretRequest, TurnUsage, WebEvent } from "../../api/contracts";
import { text, type Locale } from "../i18n/locale";

export type ToolLifecycle = {
  id: string;
  name: string;
  argumentsPreview: string;
  arguments: string;
  progress: string;
  output: string;
  status: "preparing" | "running" | "completed" | "failed";
  /**
   * 调用开始与结束的毫秒时间戳。
   *
   * 取自事件自带的 timestamp，因此实时流与历史回放走同一套口径。
   * 结束时间缺席表示仍在执行，折叠行据此在"已耗时"与"总耗时"之间切换。
   */
  startedAtMs?: number;
  endedAtMs?: number;
  /**
   * 本次调用获批的权限决定。
   *
   * 权限请求与随后的工具调用是同一次操作，分成两张卡片会让信息重复、
   * 还会打断相邻工具卡的分组折叠，因此获批后并入工具卡一起展示。
   * 被拒绝的请求不会产生工具调用，仍以独立的权限卡呈现。
   */
  permission?: PermissionDecision;
};

export type LiveMessagePart =
  | { id: string; type: "reasoning"; source: string; startedAt: string; endedAt?: string }
  | { id: string; type: "text"; source: string }
  | { id: string; type: "automatic_input"; kind: string; source: string }
  | { id: string; type: "tool"; tool: ToolLifecycle }
  | { id: string; type: "permission"; request: PermissionRequest; decision?: PermissionDecision }
  | { id: string; type: "engine_ready"; engine: string; version: string }
  | { id: string; type: "question"; pending: PendingQuestion; response?: QuestionResponse }
  | { id: string; type: "ssh_secret"; request: SshSecretRequest; resolved?: boolean }
  | { id: string; type: "compaction"; status: "running" | "completed"; turnCount: number; model?: string; applied?: boolean; summary?: string; error?: RunErrorDetail };

export type RunErrorDetail = {
  message: string;
  detail: string;
};

export type LiveRunState = {
  runId: string | null;
  sessionId: string | null;
  status: "idle" | "queued" | "waiting_response" | "waiting_external" | "waiting_permission" | "waiting_question" | "waiting_ssh_secret" | "thinking" | "working" | "compacting" | "reconnecting";
  /** 传输层自动重连的当前尝试次数（从 1 起）；非重连态为 null。 */
  reconnectAttempt: number | null;
  /** 传输层自动重连的最大尝试次数；非重连态为 null。 */
  reconnectMaxAttempts: number | null;
  /** 本轮开始时间戳（毫秒），用于状态行展示已用时长。 */
  startedAtMs: number | null;
  /** 本轮请求使用的模型；附着到已有运行时未知，落库后由时间线补齐 */
  model: string | null;
  userInput: string;
  imageUrls: string[];
  /** 排队插入点；非排队运行为 turn */
  insertAt: QueueInsertAt;
  content: string;
  reasoning: string;
  tools: ToolLifecycle[];
  parts: LiveMessagePart[];
  error: string | null;
  errorDetail: string | null;
  completed: boolean;
  /** 本轮耗时（毫秒），从首次思考/正文到结束 */
  durationMs: number | null;
  /** 本轮首字延迟（毫秒），从发请求到首个思考/正文 token */
  ttftMs: number | null;
  /** 本轮全部模型请求的汇总 token 与缓存用量 */
  usage: TurnUsage | null;
  /**
   * 已获批但尚未并入工具卡的权限决定。
   *
   * agent 的事件顺序是 permission.resolved 先于 tool.call，
   * 这里暂存到对应工具调用创建为止。
   */
  grantedPermission?: { requestId: string; tool: string; decision: PermissionDecision };
};

export type RunAction =
  | { type: "start"; runId: string; sessionId: string; userInput: string; imageUrls?: string[]; model?: string; insertAt?: QueueInsertAt }
  | { type: "attach"; runId: string; sessionId: string; userInput: string; imageUrls?: string[]; model?: string; insertAt?: QueueInsertAt }
  | { type: "event"; event: WebEvent }
  | { type: "reset" };

export const initialRunState: LiveRunState = {
  runId: null,
  sessionId: null,
  status: "idle",
  reconnectAttempt: null,
  reconnectMaxAttempts: null,
  startedAtMs: null,
  model: null,
  userInput: "",
  imageUrls: [],
  insertAt: "turn",
  content: "",
  reasoning: "",
  tools: [],
  parts: [],
  error: null,
  errorDetail: null,
  completed: false,
  durationMs: null,
  ttftMs: null,
  usage: null
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
    return {
      ...initialRunState,
      runId: action.runId,
      sessionId: action.sessionId,
      userInput: action.userInput,
      imageUrls: action.imageUrls ?? [],
      insertAt: action.insertAt ?? "turn",
      model: action.model ?? null,
      status: "waiting_response",
      startedAtMs: Date.now(),
      durationMs: null,
      ttftMs: null
    };
  }
  const { event } = action;
  const payload = event.payload;
  switch (event.type) {
    case "status.changed": {
      const next = String(payload.status) as LiveRunState["status"];
      // 正文已开始后忽略回退到 thinking，避免 Codex 晚到推理状态卡住指示器
      if (next === "thinking" && (state.content || state.status === "working")) {
        return state;
      }
      if (next === "reconnecting") {
        const attempt = Number(payload.attempt);
        const maxAttempts = Number(payload.max_attempts);
        return {
          ...state,
          status: "reconnecting",
          reconnectAttempt: Number.isFinite(attempt) && attempt > 0 ? attempt : null,
          reconnectMaxAttempts: Number.isFinite(maxAttempts) && maxAttempts > 0 ? maxAttempts : null
        };
      }
      // 离开重连态时清掉进度，避免旧 attempt 粘在后续 Working 上
      return {
        ...state,
        status: next,
        reconnectAttempt: null,
        reconnectMaxAttempts: null
      };
    }
    case "run.queued":
      return {
        ...state,
        status: "queued",
        insertAt: parseQueueInsertAt(payload.insert_at) ?? state.insertAt
      };
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
    case "message.content.delta": {
      const withClock = markFirstOutput(state, event.timestamp);
      return appendTextPart(closeActiveReasoning(withClock, event.timestamp), event.sequence, String(payload.text ?? ""));
    }
    case "message.reasoning.delta": {
      const withClock = markFirstOutput(state, event.timestamp);
      return appendReasoningPart(withClock, event.sequence, event.timestamp, String(payload.text ?? ""));
    }
    case "tool.call.preparing":
      return upsertTool(closeActiveReasoning(state, event.timestamp), String(payload.tool_id), {
        name: String(payload.name ?? "tool"),
        argumentsPreview: String(payload.arguments_preview ?? ""),
        status: "preparing",
        startedAtMs: eventTimeMs(event.timestamp)
      });
    case "tool.call.started":
      return upsertTool(closeActiveReasoning(state, event.timestamp), String(payload.tool_id), {
        name: String(payload.name ?? "tool"),
        arguments: String(payload.arguments ?? ""),
        argumentsPreview: String(payload.arguments ?? ""),
        status: "running",
        startedAtMs: eventTimeMs(event.timestamp)
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
        status: payload.ok === false ? "failed" : "completed",
        endedAtMs: eventTimeMs(event.timestamp)
      });
    case "engine.ready":
      // 外部内核连上后的运行时证据：名称与版本来自 ACP 握手响应，
      // 只有真正拉起子进程才拿得到，用来分辨本轮由谁执行
      return {
        ...state,
        parts: [...state.parts, {
          id: `engine-${event.sequence}`,
          type: "engine_ready",
          engine: String(payload.engine ?? "ACP agent"),
          version: String(payload.version ?? "")
        }]
      };
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
    case "ssh.secret.requested":
      return upsertSshSecretPart({
        ...closeActiveReasoning(state, event.timestamp),
        status: "waiting_ssh_secret"
      }, payload as unknown as SshSecretRequest);
    case "ssh.secret.resolved":
      return resolveSshSecretPart(
        { ...state, status: "working" },
        String(payload.request_id)
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
    case "run.failed": {
      const message = String(payload.message ?? text(locale, "Run failed", "运行失败"));
      return {
        ...closeActiveReasoning(state, event.timestamp),
        error: message,
        errorDetail: nonEmptyDetail(payload.detail) ?? message,
        status: "idle",
        completed: true
      };
    }
    case "run.interrupted": {
      // 1. 优先使用事件 detail
      // 2. 缺省时使用中性说明，避免在没有证据时归因给用户
      const fallbackDetail = state.content
        ? text(
            locale,
            "Generation stopped before the response finished. Partial content above was preserved.",
            "生成在完整结束前被停止，上方已保留部分内容。"
          )
        : text(
            locale,
            "The run was interrupted before completion; no confirmed cause was reported.",
            "运行在完成前中断，未收到可确认的中断原因。"
          );
      return {
        ...closeActiveReasoning(state, event.timestamp),
        // 中断事件可能先于工具结果到达，主动结束仍在加载的工具卡，避免界面继续旋转
        tools: state.tools.map((tool) =>
          tool.status === "preparing" || tool.status === "running"
            ? {
                ...tool,
                status: "failed" as const,
                output: tool.output || text(locale, "Tool call interrupted", "工具调用已中断")
              }
            : tool
        ),
        error: state.content
          ? text(locale, "The response was interrupted; generated content was preserved", "响应已中断，已保留生成内容")
          : text(locale, "The run was interrupted", "运行已中断"),
        errorDetail: nonEmptyDetail(payload.detail) ?? fallbackDetail,
        status: "idle",
        completed: true
      };
    }
    case "run.completed": {
      const durationMs = typeof payload.duration_ms === "number" ? payload.duration_ms : state.durationMs;
      const ttftMs = typeof payload.ttft_ms === "number" ? payload.ttft_ms : state.ttftMs;
      return {
        ...closeActiveReasoning(state, event.timestamp),
        status: "idle",
        completed: true,
        durationMs: durationMs ?? null,
        ttftMs: ttftMs ?? null,
        usage: parseTurnUsage(payload.usage)
      };
    }
    case "session.summary": {
      const durationMs = typeof payload.duration_ms === "number" ? payload.duration_ms : state.durationMs;
      const ttftMs = typeof payload.ttft_ms === "number" ? payload.ttft_ms : state.ttftMs;
      return {
        ...state,
        durationMs: durationMs ?? state.durationMs,
        ttftMs: ttftMs ?? state.ttftMs
      };
    }
    default:
      return state;
  }
}

/**
 * 校验运行完成事件中的单轮用量。
 *
 * @param value 事件载荷中的 usage 字段
 * @returns 字段完整时返回单轮用量，否则返回空
 */
function parseTurnUsage(value: unknown): TurnUsage | null {
  if (!value || typeof value !== "object") return null;
  const usage = value as Record<string, unknown>;
  const read = (key: string) => typeof usage[key] === "number" ? Math.max(0, Number(usage[key])) : 0;
  const promptTokens = read("prompt_tokens");
  const completionTokens = read("completion_tokens");
  const totalTokens = read("total_tokens");
  if (promptTokens === 0 && completionTokens === 0 && totalTokens === 0) return null;
  return {
    prompt_tokens: promptTokens,
    completion_tokens: completionTokens,
    total_tokens: totalTokens,
    cache_read_tokens: read("cache_read_tokens"),
    cache_write_tokens: read("cache_write_tokens")
  };
}

/**
 * 从事件载荷中读取非空错误详情。
 *
 * @param value 待检查的事件字段
 * @returns 去除首尾空白后的详情；无有效文本时返回空
 */
function nonEmptyDetail(value: unknown): string | null {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

/**
 * 首次思考/正文输出时校准本轮计时起点。
 *
 * @param state 当前运行状态
 * @param timestamp 事件时间戳
 * @returns 更新后的状态
 */
function markFirstOutput(state: LiveRunState, timestamp: string): LiveRunState {
  // 等待阶段的 startedAtMs 是请求开始时间；首次输出后改用输出时间，与 CLI/TUI 一致
  if (state.status === "thinking" || state.status === "working") {
    // 已在输出阶段时不再覆盖
    if (state.parts.some((part) => part.type === "reasoning" || part.type === "text")) {
      return state;
    }
  }
  const hasOutput = state.parts.some((part) => part.type === "reasoning" || part.type === "text");
  if (hasOutput) return state;
  const ms = Date.parse(timestamp);
  if (!Number.isFinite(ms)) return state;
  return { ...state, startedAtMs: ms };
}


function resolvePermissionPart(state: LiveRunState, requestId: string, decision: PermissionDecision): LiveRunState {
  const next = {
    ...state,
    parts: state.parts.map((part) => part.type === "permission" && part.request.id === requestId
      ? { ...part, decision }
      : part)
  };
  // 1. 拒绝不会产生工具调用，权限卡独立保留展示结论
  if (decision.decision !== "allow") return next;
  // 2. 放行后紧接着就是对应的工具调用，暂存决定等待并入
  const granted = next.parts.find(
    (part) => part.type === "permission" && part.request.id === requestId
  );
  if (granted?.type !== "permission") return next;
  return {
    ...next,
    grantedPermission: { requestId, tool: granted.request.tool, decision }
  };
}

/**
 * 把已获批的权限并入刚创建的工具调用。
 *
 * 仅在工具名一致时并入：agent 的事件顺序是先请求权限再调用同一个工具，
 * 名称不符说明中间插入了别的调用，此时宁可保留独立权限卡也不错误关联。
 *
 * @param state 当前运行状态
 * @param toolId 新建工具调用的标识
 * @param toolName 新建工具调用的名称
 * @returns 并入结果；无待并入权限时原样返回
 */
function attachGrantedPermission(
  state: LiveRunState,
  toolId: string,
  toolName: string
): LiveRunState {
  const granted = state.grantedPermission;
  if (!granted || granted.tool !== toolName) return state;
  return {
    ...state,
    grantedPermission: undefined,
    tools: state.tools.map((tool) =>
      tool.id === toolId ? { ...tool, permission: granted.decision } : tool
    ),
    // 权限已并入工具卡，移除独立的权限 part 避免同一次操作出现两张卡
    parts: state.parts
      .filter((part) => !(part.type === "permission" && part.request.id === granted.requestId))
      .map((part) =>
        part.type === "tool" && part.tool.id === toolId
          ? { ...part, tool: { ...part.tool, permission: granted.decision } }
          : part
      )
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

function upsertSshSecretPart(state: LiveRunState, request: SshSecretRequest): LiveRunState {
  const id = `ssh-secret-${request.id}`;
  const existing = state.parts.findIndex((part) => part.type === "ssh_secret" && part.request.id === request.id);
  if (existing === -1) return { ...state, parts: [...state.parts, { id, type: "ssh_secret", request }] };
  return {
    ...state,
    parts: state.parts.map((part, index) => index === existing ? { id, type: "ssh_secret" as const, request } : part)
  };
}

function resolveSshSecretPart(state: LiveRunState, requestId: string): LiveRunState {
  return {
    ...state,
    parts: state.parts.map((part) => part.type === "ssh_secret" && part.request.id === requestId
      ? { ...part, resolved: true }
      : part)
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

/**
 * 将事件时间戳解析为毫秒数。
 *
 * 事件时间由后端统一写入，用它而非本地时钟，历史回放与实时流才有同一套口径。
 *
 * @param timestamp 事件时间戳字符串
 * @returns 毫秒时间戳；无法解析时返回 undefined
 */
function eventTimeMs(timestamp: string | undefined): number | undefined {
  if (!timestamp) return undefined;
  const parsed = Date.parse(timestamp);
  return Number.isNaN(parsed) ? undefined : parsed;
}

function parseRunError(value: unknown): RunErrorDetail | undefined {  if (!value || typeof value !== "object") return undefined;
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
    const created = {
      ...state,
      tools: [...state.tools, tool],
      parts: [...state.parts, { id: `tool-${id}`, type: "tool" as const, tool }]
    };
    return attachGrantedPermission(created, id, tool.name);
  }
  const existing = state.tools[index];
  if (patch.name && existing.name !== "tool" && existing.name !== "invoke_tool" && patch.name !== existing.name) {
    const forkedId = `${id}-${patch.name}`;
    return upsertTool(state, forkedId, patch);
  }
  // 开始时间只认第一次：preparing 已经打点后，started 事件不应把起点后移
  const merged = { ...existing, ...patch };
  if (existing.startedAtMs !== undefined) merged.startedAtMs = existing.startedAtMs;
  const tools = state.tools.map((tool, toolIndex) => toolIndex === index ? merged : tool);
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
  // 正文到达后，从等待/思考态切到 working，避免指示器卡在“正在整理思路”
  const status = !state.completed && (state.status === "thinking" || state.status === "waiting_response")
    ? "working"
    : state.status;
  return { ...state, content: state.content + text, parts, status };
}

function appendReasoningPart(state: LiveRunState, sequence: number, timestamp: string, text: string): LiveRunState {
  const last = state.parts.at(-1);
  const parts = last?.type === "reasoning" && !last.endedAt
    ? state.parts.map((part, index) => index === state.parts.length - 1 && part.type === "reasoning" ? { ...part, source: part.source + text } : part)
    : [...state.parts, { id: `reasoning-${sequence}`, type: "reasoning" as const, source: text, startedAt: timestamp }];
  // 推理增量到达且尚未进入正文时，从等待态切到 thinking
  const status = !state.completed && !state.content && state.status === "waiting_response"
    ? "thinking"
    : state.status;
  return { ...state, reasoning: state.reasoning + text, parts, status };
}

function closeActiveReasoning(state: LiveRunState, timestamp: string): LiveRunState {
  const last = state.parts.at(-1);
  if (last?.type !== "reasoning" || last.endedAt) return state;
  return {
    ...state,
    parts: state.parts.map((part, index) => index === state.parts.length - 1 && part.type === "reasoning" ? { ...part, endedAt: timestamp } : part)
  };
}

/**
 * 解析排队插入点；无法识别时返回空，由调用方保留原值。
 *
 * @param value 事件或 API 中的插入点
 * @returns 合法插入点
 */
export function parseQueueInsertAt(value: unknown): QueueInsertAt | undefined {
  return value === "request" || value === "turn" ? value : undefined;
}

