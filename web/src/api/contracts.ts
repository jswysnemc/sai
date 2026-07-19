export type RunMode = "plan" | "audited" | "yolo";

export type PermissionConfig = {
  default_mode: RunMode;
  tui_mode?: RunMode;
  cli_mode?: RunMode;
};

export type PermissionAuditEvent = {
  timestamp_ms: number;
  session_id: string;
  mode: RunMode;
  tool: string;
  decision: "requested" | "approved" | "allowed" | "denied" | "completed" | "failed";
  arguments: Record<string, unknown>;
  detail?: string | null;
};

export type PermissionRequest = {
  id: string;
  session_id: string;
  tool: string;
  arguments: string;
};

export type PermissionDecision =
  | { decision: "allow" }
  | { decision: "deny"; reply?: string | null };

export type QuestionOption = {
  label: string;
  description: string;
};

export type QuestionPrompt = {
  header: string;
  question: string;
  options: QuestionOption[];
  multiple?: boolean;
  custom?: boolean;
};

export type QuestionRequestPayload = {
  questions: QuestionPrompt[];
};

export type PendingQuestion = {
  id: string;
  session_id: string;
  request: QuestionRequestPayload;
};

export type QuestionAnswers = string[][];

export type QuestionResponse =
  | { status: "answered"; data: QuestionAnswers }
  | { status: "cancelled" }
  | { status: "unavailable"; data: string };
export type Workspace = {
  id: string;
  name: string;
  path: string;
  last_opened_at: string;
};

export type WorkspaceList = {
  active_id: string;
  workspaces: Workspace[];
};

export type DirectoryEntry = {
  name: string;
  path: string;
  git_repository: boolean;
};

export type DirectoryListing = {
  current: string;
  parent?: string | null;
  roots: DirectoryEntry[];
  entries: DirectoryEntry[];
};

export type Session = {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  active: boolean;
};

export type WorkspaceSessions = {
  workspace_id: string;
  workspace_name: string;
  workspace_path: string;
  active: boolean;
  sessions: Session[];
};

export type UndoSessionResult = {
  removed: number;
  prompt?: string | null;
  worktree_restored: boolean;
};

export type ContextRollbackResult = {
  removed: number;
  prompt?: string | null;
};

export type HistoryEntry = {
  timestamp: string;
  role: string;
  content: string;
  reasoning?: string | null;
};

export type TimelineMessage = {
  timestamp: string;
  content: string;
  reasoning?: string | null;
};

export type TimelineToolEntry = {
  id: string;
  name: string;
  arguments: string;
  status: "running" | "completed" | "failed";
  output: string;
  ok?: boolean | null;
  error?: string | null;
  result_ref?: string | null;
  original_chars?: number | null;
  created_at: string;
  completed_at?: string | null;
  permission?: PermissionDecision | null;
};

export type SessionTimelineTurn = {
  turn_id: string;
  seq: number;
  status: "running" | "completed" | "interrupted";
  automatic: boolean;
  user: TimelineMessage;
  assistant: TimelineMessage;
  tools: TimelineToolEntry[];
};

export type SessionTimelineCompaction = {
  applied: boolean;
  turn_count: number;
  summary: string;
  created_at: string;
  reason: "auto" | "manual" | "legacy" | string;
};

export type SessionTimeline = {
  turns: SessionTimelineTurn[];
  compaction?: SessionTimelineCompaction | null;
};

export type FileNode = {
  name: string;
  path: string;
  kind: "file" | "directory" | "symlink";
  children: FileNode[];
};

export type FileContent = {
  path: string;
  content: string;
  size: number;
  modified_at?: number | null;
};

export type GitDiff = {
  repository: boolean;
  branch: string;
  status: string;
  files: GitFileStatus[];
  diff: string;
};

export type GitFileStatus = {
  path: string;
  index_status: string;
  worktree_status: string;
};

export type GitDirtyCounts = {
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
};

export type GitStatusEntry = {
  path: string;
  old_path?: string | null;
  index_status: string;
  worktree_status: string;
  kind: string;
  staged: boolean;
  conflicted: boolean;
  untracked: boolean;
};

export type GitInProgressOperation = {
  kind: "merge" | "rebase" | "cherry_pick" | "revert" | string;
  can_continue: boolean;
  can_skip: boolean;
  can_abort: boolean;
};

export type GitRepositoryState = {
  repo_root: string;
  workdir: string;
  head: string;
  has_commits: boolean;
  upstream: string;
  remote_name: string;
  remote_url: string;
  ahead: number;
  behind: number;
  stash_count: number;
  dirty_counts: GitDirtyCounts;
  entries: GitStatusEntry[];
  operation: GitInProgressOperation | null;
  status: string;
  error?: string | null;
};

export type GitWorktree = {
  path: string;
  head: string;
  branch: string;
  bare: boolean;
  detached: boolean;
  locked: boolean;
  prunable: boolean;
  current: boolean;
};

export type GitRepositorySummary = {
  root: string;
  name: string;
  head: string;
  ahead: number;
  behind: number;
  changed: number;
  status: string;
  error?: string | null;
  worktrees: GitWorktree[];
};

export type GitRepositoriesResponse = {
  workspace_root: string;
  repositories: GitRepositorySummary[];
};

export type GitRepositoryStatusesResponse = {
  repositories: GitRepositoryState[];
};

export type GitBranch = {
  name: string;
  full_name: string;
  kind: string;
  current: boolean;
  upstream: string;
  ahead: number;
  behind: number;
};

export type GitBranchesResponse = {
  state: GitRepositoryState;
  branches: GitBranch[];
};

export type GitDiffResponse = {
  base_ref: string;
  head_ref: string;
  mode: string;
  files: string[];
  patch: string;
  stat: string;
  truncated: boolean;
  binary_files: string[];
};

export type GitCommitFile = {
  path: string;
  old_path?: string | null;
  status: string;
  kind: string;
};

export type GitCommitSummary = {
  sha: string;
  short_sha: string;
  parents: string[];
  refs: string[];
  subject: string;
  author_name: string;
  author_email: string;
  author_date: string;
  files: GitCommitFile[];
  file_count: number;
  local_only: boolean;
  remote_only: boolean;
};

export type GitLogResponse = {
  state: GitRepositoryState;
  commits: GitCommitSummary[];
  history_base_ref: string;
  history_remote_ref: string;
  history_ahead: number;
  history_behind: number;
  merge_base: string;
};

export type GitCommitDetails = {
  sha: string;
  short_sha: string;
  subject: string;
  body: string;
  author_name: string;
  author_email: string;
  author_date: string;
  files: GitCommitFile[];
  file_count: number;
  files_changed: number;
  insertions: number;
  deletions: number;
  stat: string;
  remote_name: string;
  remote_url: string;
};

export type GitCommitDetailsResponse = {
  state: GitRepositoryState;
  commit: GitCommitDetails;
};

export type GitOperationResponse = {
  ok: boolean;
  state: GitRepositoryState;
  stdout: string;
  stderr: string;
  message: string;
};

export type GitStashEntry = {
  reference: string;
  sha: string;
  subject: string;
  created_at: string;
};

export type GitTag = {
  name: string;
  sha: string;
  created_at: string;
  subject: string;
};

export type GitRemote = {
  name: string;
  fetch_url: string;
  push_url: string;
};

export type GitRepositoryResources = {
  state: GitRepositoryState;
  stashes: GitStashEntry[];
  tags: GitTag[];
  remotes: GitRemote[];
};

export type GitConflictContent = {
  state: GitRepositoryState;
  path: string;
  base: string | null;
  ours: string | null;
  theirs: string | null;
  current: string;
};

export type FileMutation = {
  path: string;
  kind: "file" | "directory";
};

export type PromptKind = "personas" | "identities";

export type PromptSummary = {
  name: string;
};

export type PromptDocument = PromptSummary & {
  content: string;
};

export type GatewayStatus = {
  id: string;
  title: string;
  enabled: boolean;
  task_id?: string | null;
  status: string;
  pid?: number | null;
};

export type WeixinLoginPhase =
  | "waiting"
  | "scanned"
  | "need_verify_code"
  | "confirmed"
  | "expired"
  | "failed";

export type WeixinLoginAccount = {
  account_id: string;
  base_url: string;
  cdn_base_url: string;
  user_id?: string | null;
};

export type WeixinLoginSnapshot = {
  session_id: string;
  phase: WeixinLoginPhase;
  qrcode_content: string;
  qrcode_svg: string;
  message?: string | null;
  account?: WeixinLoginAccount | null;
};

export type CronJob = {
  id: string;
  name: string;
  prompt: string;
  session_id: string;
  interval_seconds?: number | null;
  next_run_at: number;
  enabled: boolean;
  failure_count: number;
  last_error?: string | null;
};

export type CreateCronJobRequest = {
  name: string;
  prompt: string;
  session_id: string;
  run_at: number;
  interval_seconds?: number | null;
};

export type UpdateCronJobRequest = {
  enabled: boolean;
};

export type ProviderConfig = {
  id: string;
  display_name: string;
  base_url: string;
  api_key?: string;
  protocol?: string;
  models?: string[];
  default_model?: string;
  thinking_level?: string;
  thinking_format?: string;
  timeout_seconds?: number;
  temperature?: number;
  anthropic_max_tokens?: number;
  extra_body?: string;
  model_context_chars?: Record<string, number>;
  model_metadata?: Record<string, ModelMetadata>;
  [key: string]: unknown;
};

export type ModelMetadata = {
  context_chars?: number;
  max_output_tokens?: number;
  tools_enabled?: boolean;
  tags?: string[];
  web_search_tool_mode?: "enabled" | "hide_builtin" | "rename_local";
};

export type QqGatewayConfig = {
  enabled: boolean;
  transport: string;
  listen: string;
  base_url: string;
  token: string;
  app_id: string;
  client_secret: string;
  [key: string]: unknown;
};

export type WeixinGatewayConfig = {
  enabled: boolean;
  base_url: string;
  cdn_base_url: string;
  bot_type: string;
  token: string;
  account: string;
  bot_agent: string;
  [key: string]: unknown;
};

export type GatewayConfig = {
  qq: QqGatewayConfig;
  weixin: WeixinGatewayConfig;
  [key: string]: unknown;
};

export type TerminalConfig = {
  shell: string;
  [key: string]: unknown;
};

export type ContextConfig = {
  default_max_chars: number;
  compaction_provider_id?: string;
  compaction_model?: string;
  [key: string]: unknown;
};

export type ScmConfig = {
  default_view_mode: "list" | "tree";
  count_badge: "all" | "focused" | "off";
};

export type GitConfig = {
  auto_repository_detection: boolean;
  untracked_changes: "mixed" | "separate" | "hidden";
  enable_smart_commit: boolean;
  suggest_smart_commit: boolean;
  confirm_sync: boolean;
  confirm_force_push: boolean;
  confirm_empty_commits: boolean;
  post_commit_command: "none" | "push" | "sync";
  show_action_button: boolean;
  detect_worktrees: boolean;
  detect_worktrees_limit: number;
  autofetch: boolean;
  branch_random_name: {
    enable: boolean;
  };
};

export type AppConfig = {
  active_provider: string;
  providers: ProviderConfig[];
  permission?: PermissionConfig;
  gateways: GatewayConfig;
  agents?: AgentProfileConfig[];
  default_agent?: string | null;
  tui_agent?: string | null;
  cli_agent?: string | null;
  gateway_agent?: string | null;
  hooks?: HooksConfig;
  mcp?: McpConfig;
    subagent?: SubagentConfig;
  plugins?: Record<string, Record<string, unknown>>;
  prompt?: {
    prompts_dir?: string;
    identities_dir?: string;
    user_identity_file?: string;
    active_persona?: string;
    active_identity?: string;
    [key: string]: unknown;
  };
  terminal?: TerminalConfig;
  tools?: Record<string, unknown>;
  skills?: Record<string, unknown>;
  display?: Record<string, unknown>;
  scm?: ScmConfig;
  git?: GitConfig;
  context?: ContextConfig;
  [key: string]: unknown;
};

export type AgentProfileConfig = {
  id: string;
  name: string;
  description?: string;
  system_prompt?: string;
  enabled_tools?: string[];
  skills_full?: string[];
  skills_named?: string[];
  provider_id?: string;
  model?: string;
  thinking_level?: string;
  register_to_main?: boolean;
};

export type AgentRuntimeProfile = {
  id: string;
  name: string;
  provider_id: string;
  model: string;
  thinking_level: string;
};

export type AgentRuntimeProfilesResponse = {
  profiles: AgentRuntimeProfile[];
};

export type UpdateAgentRuntimeRequest = {
  provider_id: string;
  model: string;
  thinking_level: string;
};

export type SubagentProfileConfig = {
  id: string;
  name: string;
  description?: string;
  system_prompt?: string;
  provider_id?: string;
  model?: string;
  thinking_level?: string;
  exposed?: boolean;
};

export type SubagentConfig = {
  provider_id?: string;
  model?: string;
  thinking_level?: string;
  default_profile?: string;
  profiles?: SubagentProfileConfig[];
};

export type ConfigResponse = {
  config: AppConfig;
  secret_sentinel: string;
};

export type ProviderModelsResponse = {
  models: string[];
  metadata: Record<string, {
    provider: string;
    context_chars?: number | null;
    max_output_tokens?: number | null;
    tags?: string[];
  }>;
};

export type RunModelSelection = {
  providerId: string;
  model: string;
};

export type ThinkingLevel = "auto" | "max" | "xhigh" | "high" | "medium" | "low" | "none";

export type RunInfo = {
  run_id: string;
  workspace_id: string;
  session_id: string;
  input?: string;
  image_urls?: string[];
  status?: "queued" | "running" | "completed" | "interrupted" | "failed";
  discard_user_turn?: boolean;
  restore_input?: string | null;
};

export type ActiveRunsResponse = {
  run?: RunInfo | null;
  runs: RunInfo[];
};

export type WebEvent = {
  sequence: number;
  run_id: string;
  workspace_id: string;
  session_id: string;
  timestamp: string;
  type: string;
  payload: Record<string, unknown>;
};

export type TerminalInfo = {
  id: string;
  title: string;
  cols: number;
  rows: number;
};

export type BackgroundTask = {
  id: string;
  label: string;
  command: string;
  cwd: string;
  pid: number;
  status: string;
  started_at: number;
  updated_at: number;
  timeout_seconds: number;
};

export type BackgroundTaskOutput = {
  task: BackgroundTask;
  stdout?: string | null;
  stderr?: string | null;
  stdout_truncated: boolean;
  stderr_truncated: boolean;
  tail_lines: number;
};

export type TodoStatus = "pending" | "in_progress" | "completed" | "cancelled";
export type TodoItem = { id:string; text:string; status:TodoStatus; created_at:string; updated_at:string };
export type TodoHistoryBatch = { archived_at: string; items: TodoItem[] };
export type TodoSnapshot = { items: TodoItem[]; history: TodoHistoryBatch[] };

export type Subagent = {
  id:string; description:string; subagent_type:string; status:string; max_steps:number;
  started_at:number; updated_at:number; step:number; phase?:string; last_tool?:string;
  result?:string; error?:string; stats?:Record<string, unknown>;
};

export type SubagentTimelineEntry =
  | { kind:"tool"; step:number; name:string; args_preview:string; ok?:boolean|null; output_preview?:string|null }
  | { kind:"text"; text:string }
  | { kind:"reasoning"; text:string };

export type SubagentDetail = Subagent & { timeline: SubagentTimelineEntry[] };

export type SystemUsage = {
  session: {
    id: string;
    requests: number;
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    turn_count: number;
    context_prompt_tokens: number;
    context_window_tokens: number;
    context_token_ratio: number;
    tool_calls: number;
    checkpoint_count: number;
    compacted_turns: number;
    latest_checkpoint_at?: string | null;
    latest_checkpoint_reason?: "auto" | "manual" | "legacy" | null;
    compaction_warning?: string | null;
  };
  process: {
    pid: number;
    uptime_seconds: number;
    rss_bytes?: number | null;
    cpu_percent: number;
  };
  runtime: {
    active_run: boolean;
    terminal_count: number;
  };
};


export type MemoryEntry = {
  id: number;
  kind: "fact" | "episode";
  content: string;
  source: string;
  status: string;
  strength?: number;
  confidence?: number;
  recall_count?: number;
  created_at: string;
  updated_at: string;
  has_markdown?: boolean;
  markdown_path?: string;
};

export type MemoryStorageStats = {
  mode?: string;
  markdown_facts?: number;
  markdown_episodes?: number;
  fts?: {
    facts?: number;
    facts_trigram?: number;
    episodes?: number;
    episodes_trigram?: number;
    ready?: boolean;
  };
};

export type MemoryStats = {
  ok?: boolean;
  data_db?: string;
  state_db?: string;
  files_dir?: string;
  skills_dir?: string;
  facts?: number;
  episodes?: number;
  unprocessed_pending_events?: number;
  total_pending_events?: number;
  skill_records?: number;
  skill_dirs?: number;
  evicted_turns?: number;
  storage?: MemoryStorageStats;
};

export type MemorySearchHit = {
  id: number;
  content: string;
  score: number;
  timestamp: string;
  source: string;
};

export type MemorySearchResult = {
  ok?: boolean;
  query?: string;
  facts?: MemorySearchHit[];
  episodes?: MemorySearchHit[];
};



export type HookHttpRequest = {
  id?: string;
  url: string;
  method?: string;
  headers?: Record<string, string>;
  body?: string;
};

export type HookItem = {
  name: string;
  enabled?: boolean;
  event: string;
  kind?: string;
  script?: string;
  timeout_ms?: number | null;
  requests?: HookHttpRequest[];
};

export type HooksConfig = {
  enabled?: boolean;
  items?: HookItem[];
};

export type McpServerConfig = {
  id: string;
  enabled?: boolean;
  transport?: string;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  cwd?: string | null;
  url?: string | null;
  message_url?: string | null;
  headers?: Record<string, string>;
  timeout_ms?: number | null;
};

export type McpConfig = {
  enabled?: boolean;
  servers?: McpServerConfig[];
};

export type McpConfigResponse = {
  config: McpConfig;
  path: string;
  secret_sentinel: string;
};
