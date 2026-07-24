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
  image_urls?: string[];
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
  has_instruction_files: boolean;
  has_skills: boolean;
  has_tools: boolean;
  has_memory: boolean;
  has_dynamic: boolean;
  tool_count: number;
  agent_id?: string | null;
  sections: string[];
};

