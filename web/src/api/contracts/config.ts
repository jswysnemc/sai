import type { NotificationConfig, PermissionConfig, SessionConfig } from "./permissions";

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

export type MemoryRuntimeConfig = {
  enabled?: boolean;
  extraction_provider_id?: string;
  extraction_model?: string;
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
  auto_commit_message_enabled?: boolean;
  auto_commit_message_provider_id?: string;
  auto_commit_message_model?: string;
};

export type AppConfig = {
  active_provider: string;
  providers: ProviderConfig[];
  permission?: PermissionConfig;
  session?: SessionConfig;
  notification?: NotificationConfig;
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
  memory?: MemoryRuntimeConfig;
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
  load_instruction_files?: boolean;
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
