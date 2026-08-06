import type { PermissionDecision } from "./permissions";

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
  is_git_repository: boolean;
  active: boolean;
  sessions: Session[];
};

export type UndoSessionResult = {
  removed: number;
  prompt?: string | null;
  worktree_restored: boolean;
};

export type RestoreWorktreeResult = {
  restored: boolean;
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
  image_urls?: string[];
};

export type TimelineToolEntry = {
  id: string;
  /** 同一轮次中的工具调用顺序；旧历史响应可能缺失 */
  seq?: number;
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

export type TimelineTurnMessage = {
  id: string;
  seq: number;
  after_tool_seq: number;
  kind: "assistant" | "external_completion" | "goal_continuation" | "queued_user" | string;
  role: "assistant" | "user" | string;
  content: string;
  reasoning?: string | null;
  image_urls?: string[];
  created_at: string;
};

export type SessionTimelineTurn = {
  turn_id: string;
  seq: number;
  status: "running" | "completed" | "interrupted" | "failed";
  automatic: boolean;
  user: TimelineMessage;
  assistant: TimelineMessage;
  tools: TimelineToolEntry[];
  /** 同一模型回合中插入的完成回执、Goal 续作和排队用户消息 */
  messages?: TimelineTurnMessage[];
  /** 处理耗时毫秒；历史未记录时可能缺失 */
  duration_ms?: number | null;
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

export type SessionContextPrompt = {
  source: "session_baseline" | "live" | string;
  content: string;
  char_count: number;
  /** 预估 token 数，与后端预算口径一致 */
  token_count?: number;
  has_instruction_files: boolean;
  has_skills: boolean;
  has_tools: boolean;
  has_memory: boolean;
  has_dynamic: boolean;
  tool_count: number;
  agent_id?: string | null;
  sections: string[];
};


/** 跨会话共享的输入历史 */
export type InputHistoryResponse = {
  /** 按时间正序排列，末项为最近一次输入 */
  entries: string[];
  /** 服务端保留的条数上限 */
  limit: number;
};
