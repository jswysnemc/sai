import type {
  ConfigResponse,
  McpConfig,
  McpConfigResponse,
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
  GitConflictContent,
  GitDiff,
  GitDiffResponse,
  GitLogResponse,
  GitOperationResponse,
  GitRepositoriesResponse,
  GitRepositoryStatusesResponse,
  GitRepositoryResources,
  GitRepositoryState,
  HistoryEntry,
  PromptDocument,
  PromptKind,
  PromptSummary,
  PermissionAuditEvent,
  MemoryEntry,
  MemorySearchResult,
  MemoryStats,
  PermissionRequest,
  ProviderConfig,
  ProviderModelsResponse,
  RunMode,
  RunModelSelection,
  ThinkingLevel,
  RunInfo,
  ActiveRunsResponse,
  AgentRuntimeProfile,
  AgentRuntimeProfilesResponse,
  SessionTimeline,
  SystemUsage,
  UsageStatsQuery,
  UsageStatsResponse,
  Session,
  TerminalInfo,
  UpdateCronJobRequest,
  UpdateAgentRuntimeRequest,
  BackgroundTask,
  BackgroundTaskOutput,
  TodoItem,
  TodoSnapshot,
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
import type { GoalResponse, GoalUpdateRequest } from "./goal-contracts";
import type { GitOperationAction, GitOperationOptions } from "./git-contracts";
import type { McpToolInfo } from "./mcp-tool-contracts";
import type { ManagedSkill, ManagedSkillDocument } from "./skill-contracts";

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
    openWindow: (path: string) =>
      apiRequest<{ opened: boolean; url: string }>("/api/workspaces/open-window", {
        method: "POST",
        body: JSON.stringify({ path })
      }),
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
  goals: {
    read: (sessionId: string) =>
      apiRequest<GoalResponse>(`/api/sessions/${encodeURIComponent(sessionId)}/goal`),
    set: (sessionId: string, objective: string, tokenBudget?: number) =>
      apiRequest<GoalResponse>(`/api/sessions/${encodeURIComponent(sessionId)}/goal`, {
        method: "PUT",
        body: JSON.stringify({ objective, token_budget: tokenBudget })
      }),
    update: (sessionId: string, request: GoalUpdateRequest) =>
      apiRequest<GoalResponse>(`/api/sessions/${encodeURIComponent(sessionId)}/goal`, {
        method: "PATCH",
        body: JSON.stringify(request)
      }),
    clear: (sessionId: string) =>
      apiRequest<{ cleared: boolean }>(`/api/sessions/${encodeURIComponent(sessionId)}/goal`, {
        method: "DELETE"
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
    startGoal: (
      sessionId: string,
      mode: RunMode,
      selection?: RunModelSelection,
      thinkingLevel?: ThinkingLevel,
      agentId?: string
    ) =>
      apiRequest<RunInfo>("/api/runs", {
        method: "POST",
        body: JSON.stringify({
          kind: "goal_continuation",
          session_id: sessionId,
          agent_id: agentId,
          input: "",
          mode,
          provider_id: selection?.providerId,
          model: selection?.model,
          thinking_level: thinkingLevel
        })
      }),
    stop: (id: string) => apiRequest<{ stopped: boolean }>(`/api/runs/${id}`, { method: "DELETE" })
  },
  agents: {
    runtimeProfiles: () => apiRequest<AgentRuntimeProfilesResponse>("/api/agents/runtime"),
    updateRuntime: (agentId: string, request: UpdateAgentRuntimeRequest) =>
      apiRequest<AgentRuntimeProfile>(`/api/agents/${encodeURIComponent(agentId)}/runtime`, {
        method: "PUT",
        body: JSON.stringify(request)
      })
  },

  memory: {
    stats: () => apiRequest<MemoryStats>("/api/memory/stats"),
    list: (limit = 100) => apiRequest<{ facts: MemoryEntry[]; episodes: MemoryEntry[] }>(`/api/memory/entries?limit=${limit}`),
    search: (q: string, limit = 20, forgotten = false) =>
      apiRequest<MemorySearchResult>(`/api/memory/search?q=${encodeURIComponent(q)}&limit=${limit}&forgotten=${forgotten}`),
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
    save: (path: string, content: string, expectedVersion?: string, expectedModifiedAt?: number | null) =>
      apiRequest<FileContent>("/api/workspace/file", {
        method: "PUT",
        body: JSON.stringify({
          path,
          content,
          expected_version: expectedVersion,
          expected_modified_at: expectedModifiedAt
        })
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
    gitRepositories: () => apiRequest<GitRepositoriesResponse>("/api/workspace/git/repositories"),
    gitStatus: (repoRoot?: string) => apiRequest<GitRepositoryState>(gitUrl("/api/workspace/git/status", repoRoot)),
    gitStatuses: (repoRoots: string[]) =>
      apiRequest<GitRepositoryStatusesResponse>("/api/workspace/git/statuses", {
        method: "POST",
        body: JSON.stringify({ repo_roots: repoRoots })
      }),
    gitClone: (remoteUrl: string, parent: string, directory?: string) =>
      apiRequest<GitOperationResponse>("/api/workspace/git/clone", {
        method: "POST",
        body: JSON.stringify({ remote_url: remoteUrl, parent, directory })
      }),
    gitBranches: (repoRoot?: string) => apiRequest<GitBranchesResponse>(gitUrl("/api/workspace/git/branches", repoRoot)),
    gitLog: (limit = 50, skip = 0, repoRoot?: string) => {
      const query = gitQuery(repoRoot);
      query.set("limit", String(limit));
      query.set("skip", String(skip));
      return apiRequest<GitLogResponse>(`/api/workspace/git/log?${query}`);
    },
    gitResources: (repoRoot?: string) => apiRequest<GitRepositoryResources>(gitUrl("/api/workspace/git/resources", repoRoot)),
    gitStashDiff: (stashRef: string, repoRoot?: string) => {
      const query = new URLSearchParams({ stash_ref: stashRef });
      if (repoRoot) query.set("repo_root", repoRoot);
      return apiRequest<GitDiffResponse>(`/api/workspace/git/stash-diff?${query}`);
    },
    gitConflict: (path: string, repoRoot?: string) => {
      const query = gitQuery(repoRoot);
      query.set("path", path);
      return apiRequest<GitConflictContent>(`/api/workspace/git/conflict?${query}`);
    },
    gitCommitDetails: (commit: string, repoRoot?: string) => {
      const query = gitQuery(repoRoot);
      query.set("commit", commit);
      return apiRequest<GitCommitDetailsResponse>(`/api/workspace/git/commit?${query}`);
    },
    gitCommitDiff: (commit: string, path?: string, repoRoot?: string) => {
      const query = new URLSearchParams({ commit });
      if (repoRoot) query.set("repo_root", repoRoot);
      if (path) query.set("path", path);
      return apiRequest<GitDiffResponse>(`/api/workspace/git/commit-diff?${query}`);
    },
    gitReviewDiff: (mode: "working_tree" | "unstaged" | "staged" | "branch" = "working_tree", path?: string, repoRoot?: string) => {
      const query = new URLSearchParams({ mode });
      if (repoRoot) query.set("repo_root", repoRoot);
      if (path) query.set("path", path);
      return apiRequest<GitDiffResponse>(`/api/workspace/git/diff?${query}`);
    },
    gitFileDiff: (basePath: string, headPath: string, repoRoot?: string) => {
      const query = new URLSearchParams({ base_path: basePath, head_path: headPath });
      if (repoRoot) query.set("repo_root", repoRoot);
      return apiRequest<GitDiffResponse>(`/api/workspace/git/file-diff?${query}`);
    },
    gitOp: (action: GitOperationAction, options: GitOperationOptions = {}) =>
      apiRequest<GitOperationResponse>("/api/workspace/git/op", {
        method: "POST",
        body: JSON.stringify({ action, ...options })
      }),
    suggestCommitMessage: (repoRoot?: string) =>
      apiRequest<{ message: string }>("/api/workspace/git/suggest-commit-message", {
        method: "POST",
        body: JSON.stringify({ repo_root: repoRoot })
      })
  },
  config: {
    load: () => apiRequest<ConfigResponse>("/api/config"),
    save: (config: Record<string, unknown>) =>
      apiRequest<ConfigResponse>("/api/config", { method: "PUT", body: JSON.stringify(config) }),
    loadMcp: () => apiRequest<McpConfigResponse>("/api/config/mcp"),
    saveMcp: (config: McpConfig) =>
      apiRequest<McpConfigResponse>("/api/config/mcp", { method: "PUT", body: JSON.stringify(config) }),
    scanMcpTools: (server: import("./contracts").McpServerConfig) =>
      apiRequest<{ tools: McpToolInfo[] }>("/api/config/mcp/tools", {
        method: "POST",
        body: JSON.stringify(server)
      })
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
    list: () => apiRequest<TodoSnapshot>("/api/todos"),
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
  usage: {
    stats: (query: UsageStatsQuery = {}) => {
      const params = new URLSearchParams();
      if (query.range) params.set("range", query.range);
      if (query.source) params.set("source", query.source);
      if (query.status) params.set("status", query.status);
      if (query.provider_search) params.set("provider_search", query.provider_search);
      if (query.model_search) params.set("model_search", query.model_search);
      if (query.limit != null) params.set("limit", String(query.limit));
      if (query.offset != null) params.set("offset", String(query.offset));
      const suffix = params.size > 0 ? `?${params.toString()}` : "";
      return apiRequest<UsageStatsResponse>(`/api/usage/stats${suffix}`);
    },
    clear: () => apiRequest<{ ok: boolean }>("/api/usage/logs", { method: "DELETE" })
  },
  skills: {
    list: () => apiRequest<{ skills: Array<{ name: string; description: string }> }>("/api/skills"),
    document: (name: string) =>
      apiRequest<{ name: string; description: string; content: string }>(
        `/api/skills/${encodeURIComponent(name)}`
      ),
    managedList: () => apiRequest<{ skills: ManagedSkill[] }>("/api/skills/manage"),
    managedDocument: (id: string) =>
      apiRequest<ManagedSkillDocument>(`/api/skills/manage/${encodeURIComponent(id)}`),
    create: (directoryName: string, content: string) =>
      apiRequest<ManagedSkillDocument>("/api/skills/manage", {
        method: "POST",
        body: JSON.stringify({ directory_name: directoryName, content })
      }),
    update: (id: string, content: string) =>
      apiRequest<ManagedSkillDocument>(`/api/skills/manage/${encodeURIComponent(id)}`, {
        method: "PUT",
        body: JSON.stringify({ content })
      }),
    setEnabled: (id: string, enabled: boolean) =>
      apiRequest<ManagedSkillDocument>(`/api/skills/manage/${encodeURIComponent(id)}/enabled`, {
        method: "POST",
        body: JSON.stringify({ enabled })
      })
  }
};

/**
 * 构造包含可选仓库根目录的 Git 查询参数。
 *
 * @param repoRoot 可选仓库根目录
 * @returns Git 查询参数
 */
function gitQuery(repoRoot?: string): URLSearchParams {
  const query = new URLSearchParams();
  if (repoRoot) query.set("repo_root", repoRoot);
  return query;
}

/**
 * 构造包含可选仓库根目录的 Git GET 地址。
 *
 * @param path Git API 路径
 * @param repoRoot 可选仓库根目录
 * @returns 完整请求地址
 */
function gitUrl(path: string, repoRoot?: string): string {
  const query = gitQuery(repoRoot).toString();
  return query ? `${path}?${query}` : path;
}
