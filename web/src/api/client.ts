import type {
  ConfigResponse,
  ContextRollbackResult,
  CreateCronJobRequest,
  CronJob,
  DirectoryEntry,
  DirectoryListing,
  FileContent,
  FileMutation,
  FileNode,
  GatewayStatus,
  GitBranchesResponse,
  GitCommitDetailsResponse,
  GitDiff,
  GitDiffResponse,
  GitLogResponse,
  GitOperationResponse,
  GitRepositoryState,
  HistoryEntry,
  PromptDocument,
  PromptKind,
  PromptSummary,
  PermissionAuditEvent,
  MemoryEntry,
  PermissionRequest,
  ProviderConfig,
  ProviderModelsResponse,
  RunMode,
  RunModelSelection,
  ThinkingLevel,
  RunInfo,
  ActiveRunsResponse,
  SessionTimeline,
  SystemUsage,
  Session,
  TerminalInfo,
  UpdateCronJobRequest,
  BackgroundTask,
  BackgroundTaskOutput,
  TodoItem,
  TodoStatus,
  Subagent,
  SubagentDetail,
  Workspace,
  WorkspaceList,
  WorkspaceSessions,
  UndoSessionResult,
  WeixinLoginSnapshot
} from "./contracts";
import { ApiError } from "./api-error";
import { detectInitialLocale, text } from "../features/i18n/locale";

/** 使用 URL 启动令牌建立同源会话。 */
export async function bootstrapSession(): Promise<void> {
  const url = new URL(window.location.href);
  const token = url.searchParams.get("token");
  if (!token) return;
  const response = await fetch(`/api/auth/session?token=${encodeURIComponent(token)}`, {
    method: "POST",
    credentials: "same-origin"
  });
  if (!response.ok) throw new Error(text(detectInitialLocale(), "The Sai Web access token is invalid", "Sai Web 访问令牌无效"));
  url.searchParams.delete("token");
  window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
}

/** 发送 JSON API 请求并统一处理错误。 */
export async function apiRequest<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    credentials: "same-origin",
    ...init,
    headers: {
      ...(init?.body ? { "Content-Type": "application/json" } : {}),
      ...init?.headers
    }
  });
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { error?: string } | null;
    throw new ApiError(body?.error ?? `HTTP ${response.status}`);
  }
  return response.json() as Promise<T>;
}

export const api = {
  workspaces: {
    list: () => apiRequest<WorkspaceList>("/api/workspaces"),
    browse: (path?: string) => apiRequest<DirectoryListing>(`/api/workspaces/browse${path ? `?path=${encodeURIComponent(path)}` : ""}`),
    createDirectory: (path: string, name: string) =>
      apiRequest<DirectoryEntry>("/api/workspaces/browse/directory", {
        method: "POST",
        body: JSON.stringify({ path, name })
      }),
    add: (path: string, name?: string) =>
      apiRequest<Workspace>("/api/workspaces", {
        method: "POST",
        body: JSON.stringify({ path, name })
      }),
    switch: (id: string, closeTerminals = false) =>
      apiRequest<Workspace>(`/api/workspaces/${id}/switch${closeTerminals ? "?close_terminals=true" : ""}`, { method: "POST" }),
    rename: (id: string, name: string) =>
      apiRequest<Workspace>(`/api/workspaces/${id}`, {
        method: "PATCH",
        body: JSON.stringify({ name })
      }),
    remove: (id: string) => apiRequest<{ removed: boolean }>(`/api/workspaces/${id}`, { method: "DELETE" })
  },
  sessions: {
    list: () => apiRequest<Session[]>("/api/sessions"),
    tree: () => apiRequest<WorkspaceSessions[]>("/api/sessions/tree"),
    create: (title?: string, workspaceId?: string) =>
      apiRequest<Session>("/api/sessions", { method: "POST", body: JSON.stringify({ title, workspace_id: workspaceId }) }),
    switch: (id: string) => apiRequest<Session>(`/api/sessions/${id}/switch`, { method: "POST" }),
    rename: (id: string, title: string) =>
      apiRequest<Session>(`/api/sessions/${id}`, { method: "PATCH", body: JSON.stringify({ title }) }),
    remove: (id: string) => apiRequest<{ deleted: boolean }>(`/api/sessions/${id}`, { method: "DELETE" }),
    removeMany: (ids: string[]) =>
      apiRequest<{ deleted_ids: string[] }>("/api/sessions/bulk-delete", {
        method: "POST",
        body: JSON.stringify({ ids })
      }),
    timeline: (id: string) => apiRequest<SessionTimeline>(`/api/sessions/${id}/timeline?limit=500`),
    undo: (id: string) => apiRequest<UndoSessionResult>(`/api/sessions/${id}/undo`, { method: "POST" }),
    rollback: (id: string, turnId: string) =>
      apiRequest<ContextRollbackResult>(`/api/sessions/${id}/rollback`, {
        method: "POST",
        body: JSON.stringify({ turn_id: turnId })
      }),
    permissionAudit: (id: string) => apiRequest<PermissionAuditEvent[]>(`/api/sessions/${id}/permission-audit?limit=200`),
    fork: (id: string, turnId: string, title?: string) =>
      apiRequest<Session>(`/api/sessions/${id}/fork`, {
        method: "POST",
        body: JSON.stringify({ turn_id: turnId, title })
      }),
    compact: (id: string, selection?: RunModelSelection) =>
      apiRequest<RunInfo>(`/api/sessions/${id}/compact`, {
        method: "POST",
        body: JSON.stringify({
          provider_id: selection?.providerId,
          model: selection?.model
        })
      })
  },
  runs: {
    active: () => apiRequest<ActiveRunsResponse>("/api/runs/active"),
    interruptionRecovery: (workspaceId: string, sessionId: string) =>
      apiRequest<{ run?: RunInfo | null }>(`/api/runs/interruption-recovery?workspace_id=${encodeURIComponent(workspaceId)}&session_id=${encodeURIComponent(sessionId)}`),
    start: (
      sessionId: string,
      input: string,
      mode: RunMode,
      selection?: RunModelSelection,
      imageUrls?: string[],
      thinkingLevel?: ThinkingLevel,
      agentId?: string
    ) =>
      apiRequest<RunInfo>("/api/runs", {
        method: "POST",
        body: JSON.stringify({
          session_id: sessionId,
          agent_id: agentId,
          input,
          mode,
          provider_id: selection?.providerId,
          model: selection?.model,
          image_urls: imageUrls,
          thinking_level: thinkingLevel
        })
      }),
    stop: (id: string) => apiRequest<{ stopped: boolean }>(`/api/runs/${id}`, { method: "DELETE" })
  },

  memory: {
    stats: () => apiRequest<Record<string, unknown>>("/api/memory/stats"),
    list: (limit = 100) => apiRequest<{ facts: MemoryEntry[]; episodes: MemoryEntry[] }>(`/api/memory/entries?limit=${limit}`),
    search: (q: string, limit = 20, forgotten = false) =>
      apiRequest<Record<string, unknown>>(`/api/memory/search?q=${encodeURIComponent(q)}&limit=${limit}&forgotten=${forgotten}`),
    remember: (content: string, source = "web") =>
      apiRequest<{ ok: boolean; id: number }>("/api/memory/entries", {
        method: "POST",
        body: JSON.stringify({ content, source })
      }),
    remove: (kind: "fact" | "episode", id: number) =>
      apiRequest<{ deleted: boolean }>(`/api/memory/entries/${kind}/${id}`, { method: "DELETE" }),
    reset: () => apiRequest<{ ok: boolean }>("/api/memory/reset", { method: "POST" })
  },
  permissions: {
    decide: (request: PermissionRequest, decision: "allow" | "deny", reply?: string) =>
      apiRequest<{ accepted: boolean }>(`/api/permissions/${request.id}/decision`, {
        method: "POST",
        body: JSON.stringify({ decision, reply })
      })
  },
  questions: {
    answer: (id: string, answers: string[][]) =>
      apiRequest<{ accepted: boolean }>(`/api/questions/${id}/answer`, {
        method: "POST",
        body: JSON.stringify({ answers })
      }),
    cancel: (id: string) =>
      apiRequest<{ accepted: boolean }>(`/api/questions/${id}/answer`, {
        method: "POST",
        body: JSON.stringify({ cancelled: true })
      })
  },
  workspace: {
    tree: (path = "", depth = 5) => {
      const query = new URLSearchParams({ depth: String(depth) });
      if (path) query.set("path", path);
      return apiRequest<FileNode[]>(`/api/workspace/tree?${query.toString()}`);
    },
    file: (path: string) => apiRequest<FileContent>(`/api/workspace/file?path=${encodeURIComponent(path)}`),
    imageUrl: (path: string) => `/api/workspace/image?path=${encodeURIComponent(path)}`,
    save: (path: string, content: string, expectedModifiedAt?: number | null) =>
      apiRequest<FileContent>("/api/workspace/file", {
        method: "PUT",
        body: JSON.stringify({ path, content, expected_modified_at: expectedModifiedAt })
      }),
    create: (path: string, kind: "file" | "directory") =>
      apiRequest<FileMutation>("/api/workspace/entry", {
        method: "POST",
        body: JSON.stringify({ path, kind })
      }),
    rename: (from: string, to: string) =>
      apiRequest<FileMutation>("/api/workspace/entry", {
        method: "PATCH",
        body: JSON.stringify({ from, to })
      }),
    remove: (path: string) =>
      apiRequest<FileMutation>("/api/workspace/entry", {
        method: "DELETE",
        body: JSON.stringify({ path })
      }),
    diff: () => apiRequest<GitDiff>("/api/workspace/diff"),
    gitAction: (action: "init" | "stage" | "unstage" | "discard" | "commit", paths: string[] = [], message?: string) =>
      apiRequest<GitDiff>("/api/workspace/git", {
        method: "POST",
        body: JSON.stringify({ action, paths, message })
      }),
    gitStatus: () => apiRequest<GitRepositoryState>("/api/workspace/git/status"),
    gitBranches: () => apiRequest<GitBranchesResponse>("/api/workspace/git/branches"),
    gitLog: (limit = 50, skip = 0) =>
      apiRequest<GitLogResponse>(`/api/workspace/git/log?limit=${limit}&skip=${skip}`),
    gitCommitDetails: (commit: string) =>
      apiRequest<GitCommitDetailsResponse>(`/api/workspace/git/commit?commit=${encodeURIComponent(commit)}`),
    gitCommitDiff: (commit: string, path?: string) => {
      const query = new URLSearchParams({ commit });
      if (path) query.set("path", path);
      return apiRequest<GitDiffResponse>(`/api/workspace/git/commit-diff?${query}`);
    },
    gitReviewDiff: (mode: "working_tree" | "branch" = "working_tree", path?: string) => {
      const query = new URLSearchParams({ mode });
      if (path) query.set("path", path);
      return apiRequest<GitDiffResponse>(`/api/workspace/git/diff?${query}`);
    },
    gitOp: (action: string, options: { path?: string; message?: string; remote_url?: string } = {}) =>
      apiRequest<GitOperationResponse>("/api/workspace/git/op", {
        method: "POST",
        body: JSON.stringify({ action, ...options })
      })
  },
  config: {
    load: () => apiRequest<ConfigResponse>("/api/config"),
    save: (config: Record<string, unknown>) =>
      apiRequest<ConfigResponse>("/api/config", { method: "PUT", body: JSON.stringify(config) })
  },
  providers: {
    models: (provider: ProviderConfig) =>
      apiRequest<ProviderModelsResponse>("/api/providers/models", {
        method: "POST",
        body: JSON.stringify({ provider })
      })
  },
  prompts: {
    list: (kind: PromptKind) => apiRequest<{ items: PromptSummary[] }>(`/api/prompts/${kind}`),
    read: (kind: PromptKind, name: string) => apiRequest<PromptDocument>(`/api/prompts/${kind}/${encodeURIComponent(name)}`),
    create: (kind: PromptKind, name: string, content: string) =>
      apiRequest<PromptDocument>(`/api/prompts/${kind}`, {
        method: "POST",
        body: JSON.stringify({ name, content })
      }),
    update: (kind: PromptKind, currentName: string, name: string, content: string) =>
      apiRequest<PromptDocument>(`/api/prompts/${kind}/${encodeURIComponent(currentName)}`, {
        method: "PUT",
        body: JSON.stringify({ name, content })
      }),
    remove: (kind: PromptKind, name: string) =>
      apiRequest<{ removed: boolean }>(`/api/prompts/${kind}/${encodeURIComponent(name)}`, { method: "DELETE" })
  },
  gateways: {
    list: () => apiRequest<GatewayStatus[]>("/api/gateways"),
    start: (id: string) => apiRequest<Record<string, unknown>>(`/api/gateways/${id}/start`, { method: "POST" }),
    stop: (id: string) => apiRequest<Record<string, unknown>>(`/api/gateways/${id}/stop`, { method: "POST" }),
    weixinLogin: {
      start: (baseUrl?: string, botType?: string) =>
        apiRequest<WeixinLoginSnapshot>("/api/gateways/weixin/login", {
          method: "POST",
          body: JSON.stringify({ base_url: baseUrl, bot_type: botType })
        }),
      status: (sessionId: string) =>
        apiRequest<WeixinLoginSnapshot>(`/api/gateways/weixin/login/${encodeURIComponent(sessionId)}`),
      verify: (sessionId: string, verifyCode: string) =>
        apiRequest<WeixinLoginSnapshot>(`/api/gateways/weixin/login/${encodeURIComponent(sessionId)}/verify`, {
          method: "POST",
          body: JSON.stringify({ verify_code: verifyCode })
        })
    }
  },
  cronJobs: {
    list: () => apiRequest<CronJob[]>("/api/cron-jobs"),
    create: (request: CreateCronJobRequest) =>
      apiRequest<CronJob>("/api/cron-jobs", {
        method: "POST",
        body: JSON.stringify(request)
      }),
    update: (id: string, request: UpdateCronJobRequest) =>
      apiRequest<CronJob>(`/api/cron-jobs/${encodeURIComponent(id)}`, {
        method: "PATCH",
        body: JSON.stringify(request)
      }),
    remove: (id: string) =>
      apiRequest<{ removed: boolean }>(`/api/cron-jobs/${encodeURIComponent(id)}`, {
        method: "DELETE"
      })
  },
  terminals: {
    list: () => apiRequest<{ terminals: TerminalInfo[] }>("/api/terminals"),
    create: (cols: number, rows: number) =>
      apiRequest<TerminalInfo>("/api/terminals", { method: "POST", body: JSON.stringify({ cols, rows }) }),
    rename: (id: string, title: string) =>
      apiRequest<TerminalInfo>(`/api/terminals/${encodeURIComponent(id)}`, { method: "PATCH", body: JSON.stringify({ title }) }),
    remove: (id: string) => apiRequest<{ removed: boolean }>(`/api/terminals/${id}`, { method: "DELETE" })
  },
  backgroundTasks: {
    list: () => apiRequest<{ tasks: BackgroundTask[] }>("/api/background-tasks"),
    output: (id: string, tailLines = 200) =>
      apiRequest<BackgroundTaskOutput>(`/api/background-tasks/${encodeURIComponent(id)}/output?tail_lines=${tailLines}`),
    stop: (id: string) => apiRequest<{ task: BackgroundTask; was_running: boolean }>(`/api/background-tasks/${encodeURIComponent(id)}/stop`, { method: "POST" }),
    cleanup: (removeLogs = false) =>
      apiRequest<{ removed: string[]; remaining: number }>(`/api/background-tasks?remove_logs=${removeLogs}`, { method: "DELETE" })
  },
  todos: {
    list: () => apiRequest<TodoItem[]>("/api/todos"),
    create: (text: string) => apiRequest<TodoItem>("/api/todos", { method:"POST", body:JSON.stringify({ text }) }),
    update: (id: string, input: { text?: string; status?: TodoStatus }) => apiRequest<TodoItem>(`/api/todos/${encodeURIComponent(id)}`, { method:"PATCH", body:JSON.stringify(input) }),
    remove: (id: string) => apiRequest<TodoItem>(`/api/todos/${encodeURIComponent(id)}`, { method:"DELETE" })
  },
  subagents: {
    list: () => apiRequest<Subagent[]>("/api/subagents"),
    detail: (id: string) => apiRequest<SubagentDetail>(`/api/subagents/${encodeURIComponent(id)}`),
    cancel: (id: string) => apiRequest<Subagent>(`/api/subagents/${encodeURIComponent(id)}/cancel`, { method:"POST" })
  },
  system: {
    usage: (selection?: RunModelSelection | null) => {
      const query = new URLSearchParams();
      if (selection) {
        query.set("provider_id", selection.providerId);
        query.set("model", selection.model);
      }
      const suffix = query.size > 0 ? `?${query.toString()}` : "";
      return apiRequest<SystemUsage>(`/api/system/usage${suffix}`);
    }
  },
  skills: {
    list: () => apiRequest<{ skills: Array<{ name: string; description: string }> }>("/api/skills"),
    document: (name: string) =>
      apiRequest<{ name: string; description: string; content: string }>(
        `/api/skills/${encodeURIComponent(name)}`
      )
  }
};
