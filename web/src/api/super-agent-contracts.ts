/**
 * 【超级智能体】【Web API 契约】定义群组、Leader、Runner、任务、报告、Thread 订阅、Trigger、渠道绑定与 Watchdog 的类型。
 */

export type GroupStatus = "active" | "paused" | "archived";
export type MemberRole = "leader" | "worker";
export type MemberStatus = "idle" | "running" | "waiting" | "completed" | "failed" | "stopped";
export type MessageSenderKind = "user" | "leader" | "worker" | "system" | "watchdog";
export type MessageKind = "chat" | "dispatch" | "report" | "delivery" | "progress" | "alert" | "channel_in" | "channel_out";
export type SubscriptionMode = "owner" | "context";
export type TriggerDisposition = "queued" | "fanned_out" | "batched" | "evaluated" | "ignored" | "delivered";
export type TaskStatus = "pending" | "running" | "waiting_review" | "delivered" | "blocked" | "failed" | "stopped";
export type ReportStatus = "progress" | "waiting" | "ready" | "blocked" | "failed" | "dropped";
export type WatchdogSeverity = "healthy" | "notice" | "action_required" | "recovered";

export type SuperGroup = {
  id: string;
  name: string;
  description: string;
  workspace_path: string;
  status: GroupStatus;
  leader_member_id: string;
  default_thread_id: string;
  created_at: number;
  updated_at: number;
};

export type SuperGroupMember = {
  id: string;
  group_id: string;
  name: string;
  role: MemberRole;
  status: MemberStatus;
  specialty: string;
  subagent_type: string;
  model_label: string;
  workdir: string;
  worktree_branch?: string | null;
  active_task_id?: string | null;
  last_activity: string;
  last_heartbeat_at: number;
  created_at: number;
  updated_at: number;
};

export type SuperGroupThread = {
  id: string;
  group_id: string;
  title: string;
  channel: string;
  external_ref?: string | null;
  status: string;
  created_at: number;
  updated_at: number;
};

export type SuperThreadSubscription = {
  id: string;
  group_id: string;
  thread_id: string;
  member_id: string;
  mode: SubscriptionMode;
  created_at: number;
};

export type SuperGroupMessage = {
  seq: number;
  id: string;
  group_id: string;
  thread_id: string;
  sender_kind: MessageSenderKind;
  sender_id: string;
  sender_name: string;
  recipient_id?: string | null;
  recipient_name?: string | null;
  kind: MessageKind;
  channel_source?: string | null;
  content: string;
  summary?: string | null;
  related_task_id?: string | null;
  related_report_id?: string | null;
  created_at: number;
};

export type SuperTriggerEvent = {
  id: string;
  group_id: string;
  thread_id: string;
  source_channel: string;
  actor: string;
  content: string;
  disposition: TriggerDisposition;
  route_path: string;
  reason: string;
  target_member_ids: string[];
  metadata: Record<string, unknown>;
  created_at: number;
  updated_at: number;
};

export type SuperTask = {
  id: string;
  group_id: string;
  thread_id: string;
  title: string;
  objective: string;
  assigned_member_id?: string | null;
  assigned_member_name?: string | null;
  origin_channel: string;
  origin_target: Record<string, unknown>;
  status: TaskStatus;
  latest_report_status?: ReportStatus | null;
  latest_report_summary?: string | null;
  deliverable?: string | null;
  created_at: number;
  updated_at: number;
};

export type SuperRunnerReport = {
  id: string;
  group_id: string;
  thread_id: string;
  task_id?: string | null;
  worker_member_id: string;
  worker_name: string;
  status: ReportStatus;
  summary: string;
  details: string;
  artifacts: string[];
  needs_human_question?: string | null;
  synthesized_by_leader: boolean;
  created_at: number;
};

export type SuperChannelBinding = {
  id: string;
  group_id: string;
  channel: string;
  external_chat_id: string;
  label: string;
  admission_mode: string;
  enabled: boolean;
  last_inbound_at?: number | null;
  last_outbound_at?: number | null;
  created_at: number;
};

export type SuperDeliveryReceipt = {
  id: string;
  group_id: string;
  task_id?: string | null;
  channel: string;
  target: string;
  status: string;
  preview: string;
  error?: string | null;
  created_at: number;
};

export type SuperWatchdogEvent = {
  id: string;
  group_id?: string | null;
  severity: WatchdogSeverity;
  kind: string;
  summary: string;
  details: string;
  resolved: boolean;
  created_at: number;
};

export type SuperGroupSummary = {
  group: SuperGroup;
  leader_status: MemberStatus;
  worker_count: number;
  running_worker_count: number;
  open_task_count: number;
  pending_review_count: number;
  message_count: number;
  bound_channels: string[];
  last_message_preview?: string | null;
  last_message_at?: number | null;
};

export type SuperGroupDetail = {
  group: SuperGroup;
  members: SuperGroupMember[];
  threads: SuperGroupThread[];
  subscriptions: SuperThreadSubscription[];
  messages: SuperGroupMessage[];
  tasks: SuperTask[];
  reports: SuperRunnerReport[];
  triggers: SuperTriggerEvent[];
  channels: SuperChannelBinding[];
  deliveries: SuperDeliveryReceipt[];
};

export type SuperAgentOverview = {
  group_count: number;
  running_workers: number;
  total_workers: number;
  active_tasks: number;
  pending_synthesis_reports: number;
  delivered_tasks_24h: number;
  triggers_24h: number;
  fast_path_triggers_24h: number;
  slow_path_triggers_24h: number;
  watchdog_alerts: number;
  bound_channel_count: number;
  groups: SuperGroupSummary[];
  recent_watchdog: SuperWatchdogEvent[];
};

export type SuperAgentConfig = {
  enabled: boolean;
  channel_leader_routing: boolean;
  default_group_id?: string | null;
  leader_provider_id: string;
  leader_model: string;
  worker_provider_id: string;
  worker_model: string;
  max_concurrent_workers: number;
  leader_batch_limit: number;
  im_reply_max_words: number;
  watchdog_enabled: boolean;
  watchdog_interval_seconds: number;
  worker_stall_timeout_seconds: number;
  worker_use_worktree: boolean;
};

export type CreateGroupPayload = {
  name: string;
  description?: string;
  workspace_path?: string;
  leader_name?: string;
};

export type SpawnRunnerPayload = {
  name: string;
  specialty?: string;
  subagent_type?: string;
  task_title?: string;
  prompt: string;
  thread_id?: string;
};

export type BindChannelPayload = {
  channel: string;
  external_chat_id: string;
  label?: string;
  admission_mode?: string;
  enabled?: boolean;
};

export type ChannelInboundPayload = {
  channel: string;
  external_chat_id: string;
  actor?: string;
  content: string;
  simulate_outbound?: boolean;
  auto_dispatch?: boolean;
};
