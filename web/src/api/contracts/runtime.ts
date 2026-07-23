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
    context_breakdown?: {
      system_prompt_tokens: number;
      tools_and_agents_tokens: number;
      conversation_tokens: number;
      connectors_and_mcp_tokens: number;
      skills_tokens: number;
    } | null;
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
