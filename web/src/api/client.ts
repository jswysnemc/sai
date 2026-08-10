import type { BranchSwitchResult, SessionTurnTree } from "./turn-tree-contracts";
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
  ProviderSecretResponse,
  ProviderProbeReport,
  ProviderProbeMode,
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
  SshHost,
  SshHostInput,
  SshHostKeyPrompt,
  SshImportCandidate,
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
  RestoreWorktreeResult,
  WeixinLoginSnapshot,
  SessionContextPrompt,
  SessionDataSummary,
  SessionDataSelection
} from "./contracts";
import { ApiError } from "./api-error";
import { detectInitialLocale, text } from "../features/i18n/locale";
import type { GoalResponse, GoalUpdateRequest } from "./goal-contracts";
import type { GitOperationAction, GitOperationOptions } from "./git-contracts";
import type { McpToolInfo } from "./mcp-tool-contracts";
import type { ManagedSkill, ManagedSkillDocument } from "./skill-contracts";

/** 服务端当前启用的认证方式。 */
export async function fetchAuthMode(): Promise<{ password_required: boolean }> {
  const response = await fetch("/api/auth/mode", { credentials: "same-origin" });
  if (!response.ok) return { password_required: false };
  return (await response.json()) as { password_required: boolean };
}

/** 使用访问口令建立同源会话。 */
export async function loginWithPassword(password: string): Promise<void> {
  const response = await fetch("/api/auth/password", {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ password })
  });
  if (!response.ok) {
    throw new Error(text(detectInitialLocale(), "Incorrect password", "口令不正确"));
  }
}

/** 判断当前浏览器会话是否已通过验证。 */
export async function hasActiveSession(): Promise<boolean> {
  const response = await fetch("/api/workspaces", { credentials: "same-origin" });
  return response.ok;
}

/** 使用 URL 启动令牌建立同源会话。 */
export async function bootstrapSession(): Promise<void> {
  const url = new URL(window.location.href);
  const token = url.searchParams.get("token");
  if (!token) return;
  const response = await fetch(`/api/auth/session?token=${encodeURIComponent(token)}`, {
    method: "POST",
    credentials: "same-origin"
  });
  // 启用口令验证时令牌不再单独放行，此处失败交由登录页接管
  if (response.ok) {
    url.searchParams.delete("token");
    window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
    return;
  }
  const mode = await fetchAuthMode().catch(() => ({ password_required: false }));
  if (mode.password_required) return;
  throw new Error(text(detectInitialLocale(), "The Sai Web access token is invalid", "Sai Web 访问令牌无效"));
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
    const body = (await response.json().catch(() => null)) as { error?: string; detail?: string } | null;
    const message = body?.error ?? `HTTP ${response.status}`;
    throw new ApiError(message, body?.detail ?? message);
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
    contextPrompt: (id: string, options?: {
      agentId?: string;
      locale?: string;
      mode?: RunMode;
      selection?: RunModelSelection | null;
    }) => {
      const params = new URLSearchParams();
      if (options?.agentId) params.set("agent_id", options.agentId);
      if (options?.locale) params.set("locale", options.locale);
      if (options?.mode) params.set("mode", options.mode);
      if (options?.selection) {
        params.set("provider_id", options.selection.providerId);
        params.set("model", options.selection.model);
      }
      const query = params.toString();
      return apiRequest<SessionContextPrompt>(`/api/sessions/${id}/context-prompt${query ? `?${query}` : ""}`);
    },
    undo: (id: string) => apiRequest<UndoSessionResult>(`/api/sessions/${id}/undo`, { method: "POST" }),
    restoreWorktree: (id: string, turnId: string, paths: string[] = []) =>
      apiRequest<RestoreWorktreeResult>(`/api/sessions/${id}/restore-worktree`, {
        method: "POST",
        body: JSON.stringify({ turn_id: turnId, paths })
      }),
    rollback: (id: string, turnId: string) =>
      apiRequest<ContextRollbackResult>(`/api/sessions/${id}/rollback`, {
        method: "POST",
        body: JSON.stringify({ turn_id: turnId })
      }),
    permissionAudit: (id: string) => apiRequest<PermissionAuditEvent[]>(`/api/sessions/${id}/permission-audit?limit=200`),
    turnTree: (id: string) => apiRequest<SessionTurnTree>(`/api/sessions/${id}/turn-tree`),
    switchBranch: (id: string, turnId: string) =>
      apiRequest<BranchSwitchResult>(`/api/sessions/${id}/turn-tree/switch`, {
        method: "POST",
        body: JSON.stringify({ turn_id: turnId })
      }),
    undoToParent: (id: string, turnId: string) =>
      apiRequest<BranchSwitchResult>(`/api/sessions/${id}/turn-tree/undo`, {
        method: "POST",
        body: JSON.stringify({ turn_id: turnId })
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
  sessionData: {
    list: () => apiRequest<SessionDataSummary[]>("/api/session-data"),
    clear: (id: string) =>
      apiRequest<{ cleared: boolean }>(`/api/session-data/${encodeURIComponent(id)}/clear`, {
        method: "POST"
      }),
    clearMany: (sessions: SessionDataSelection[]) =>
      apiRequest<{ cleared: boolean; cleared_ids: string[] }>("/api/session-data/clear", {
        method: "POST",
        body: JSON.stringify({ sessions })
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
    stop: (id: string) => apiRequest<{ stopped: boolean }>(`/api/runs/${id}`, { method: "DELETE" }),
    /** 更新排队运行的正文或等待位置。 */
    updateQueue: (id: string, update: { input?: string; position?: number }) =>
      apiRequest<RunInfo>(`/api/runs/${id}/queue`, {
        method: "PATCH",
        body: JSON.stringify(update)
      })
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
  inputHistory: {
    /** 读取跨会话输入历史，与 TUI 共用同一份存储 */
    list: () => apiRequest<import("./contracts").InputHistoryResponse>("/api/input-history"),
    /** 记录一条输入历史并返回更新后的列表 */
    append: (entry: string) =>
      apiRequest<import("./contracts").InputHistoryResponse>("/api/input-history", {
        method: "POST",
        body: JSON.stringify({ entry })
      })
  },

  config: {
    load: () => apiRequest<ConfigResponse>("/api/config"),
    save: (config: Record<string, unknown>) =>
      apiRequest<ConfigResponse>("/api/config", { method: "PUT", body: JSON.stringify(config) }),
    /**
     * 按需读取指定供应商真实 API Key，响应不会进入配置查询缓存。
     *
     * @param providerId 供应商稳定标识
     * @param keyId 多密钥场景下指定要查看的密钥标识；缺省返回单值密钥
     * @returns 供应商当前实际使用的 API Key
     */
    providerSecret: (providerId: string, keyId?: string) =>
      apiRequest<ProviderSecretResponse>("/api/config/provider-secret", {
        method: "POST",
        body: JSON.stringify({ provider_id: providerId, key_id: keyId })
      }),
    loadMcp: () => apiRequest<McpConfigResponse>("/api/config/mcp"),
    rtkStatus: () => apiRequest<import("./contracts").RtkStatusResponse>("/api/config/rtk-status"),
    /** 读取当前对话内核状态，供界面标注失效信息 */
    engineStatus: () =>
      apiRequest<import("./contracts").EngineStatusResponse>("/api/config/engine-status"),
    /** 主动握手外部内核，让界面在首轮对话前拿到可选模型与思考等级 */
    engineConnect: () =>
      apiRequest<import("./contracts").EngineConnectResponse>("/api/config/engine-connect", {
        method: "POST"
      }),
    /** 丢弃已缓存的外部内核能力，界面回到未连接展示 */
    engineDisconnect: () =>
      apiRequest<{ cleared: boolean }>("/api/config/engine-disconnect", { method: "POST" }),
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
      }),
    test: (provider: ProviderConfig, model?: string, mode: ProviderProbeMode = "connection") =>
      apiRequest<ProviderProbeReport>("/api/providers/test", {
        method: "POST",
        body: JSON.stringify({ provider, model, mode })
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
    /** 建立 SSH 远程终端；主机密钥待确认时返回 host_key_prompt 而非终端 */
    createSsh: (sshHostId: string, cols: number, rows: number, passphrase?: string) =>
      apiRequest<TerminalInfo & { host_key_prompt?: SshHostKeyPrompt }>("/api/terminals", {
        method: "POST",
        body: JSON.stringify({ cols, rows, ssh_host_id: sshHostId, passphrase })
      }),
    rename: (id: string, title: string) =>
      apiRequest<TerminalInfo>(`/api/terminals/${encodeURIComponent(id)}`, { method: "PATCH", body: JSON.stringify({ title }) }),
    remove: (id: string) => apiRequest<{ removed: boolean }>(`/api/terminals/${id}`, { method: "DELETE" })
  },
  ssh: {
    list: () => apiRequest<{ hosts: SshHost[] }>("/api/ssh/hosts"),
    create: (host: SshHostInput) =>
      apiRequest<{ host: SshHost }>("/api/ssh/hosts", { method: "POST", body: JSON.stringify(host) }),
    update: (id: string, host: SshHostInput) =>
      apiRequest<{ host: SshHost }>(`/api/ssh/hosts/${encodeURIComponent(id)}`, {
        method: "PUT",
        body: JSON.stringify(host)
      }),
    remove: (id: string) =>
      apiRequest<{ removed: boolean }>(`/api/ssh/hosts/${encodeURIComponent(id)}`, { method: "DELETE" }),
    /** 扫描 ~/.ssh/config 返回可导入主机 */
    scan: () => apiRequest<{ candidates: SshImportCandidate[] }>("/api/ssh/hosts/import"),
    import: (hosts: SshHostInput[]) =>
      apiRequest<{ hosts: SshHost[] }>("/api/ssh/hosts/import", { method: "POST", body: JSON.stringify({ hosts }) }),
    /** 把用户确认过的主机密钥写入 known_hosts */
    trust: (key: SshHostKeyPrompt) =>
      apiRequest<{ trusted: boolean }>("/api/ssh/known-hosts/trust", {
        method: "POST",
        body: JSON.stringify(key)
      })
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
    usage: (
      selection?: RunModelSelection | null,
      mode?: RunMode,
      agentId?: string | null
    ) => {
      const query = new URLSearchParams();
      if (selection) {
        query.set("provider_id", selection.providerId);
        query.set("model", selection.model);
      }
      if (mode) query.set("mode", mode);
      if (agentId) query.set("agent_id", agentId);
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
