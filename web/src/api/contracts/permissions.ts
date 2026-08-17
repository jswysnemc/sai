import type { ThinkingLevel } from "./sessions";

export type RunMode = "plan" | "audited" | "auto_audit" | "yolo";

export type PermissionConfig = {
  default_mode: RunMode;
  tui_mode?: RunMode;
  cli_mode?: RunMode;
  auto_audit_provider_id?: string;
  auto_audit_model?: string;
};

export type SessionConfig = {
  new_session_provider_id?: string;
  new_session_model?: string;
  new_session_thinking_level?: ThinkingLevel;
  auto_title_enabled?: boolean;
  auto_title_provider_id?: string;
  auto_title_model?: string;
};

export type NotificationConfig = {
  enabled: boolean;
  sound: boolean;
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
  /** 是否并行自动审核 */
  auto_audit?: boolean;
};

export type PermissionAllowSource = "human" | "auto_audit";

export type PermissionDecision =
  /** 允许；reason 仅自动审核给出，是模型的放行理由，不回传给模型 */
  | { decision: "allow"; source?: PermissionAllowSource; reason?: string | null }
  /** 拒绝；reply 会作为工具输出回传给模型 */
  | { decision: "deny"; reply?: string | null };

export type QuestionOption = {
  label: string;
  description: string;
  value?: string;
};

export type QuestionPrompt = {
  header: string;
  question: string;
  options: QuestionOption[];
  multiple?: boolean;
  custom?: boolean;
  required?: boolean;
  default_answers?: string[];
  validation?: Record<string, unknown>;
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

/** SSH 交互征询类型：口令、密码、主机指纹确认、高危命令确认。 */
export type SshSecretKind =
  | "passphrase"
  | "password"
  | "sudo_password"
  | "host_key"
  | "danger_command";

/**
 * SSH 交互征询请求。
 *
 * 只描述“需要什么”，不含任何秘密。真正的口令/密码经提交端点一次性直达后端，
 * 绝不出现在事件流、消息或模型上下文里。
 */
export type SshSecretRequest = {
  id: string;
  session_id: string;
  kind: SshSecretKind;
  host_label: string;
  prompt: string;
  /** 主机指纹，仅指纹确认时给出，供用户核对 */
  fingerprint?: string | null;
  /** 指纹是否与 known_hosts 记录不一致 */
  changed?: boolean;
};

/** 提交 SSH 交互征询应答的请求体（三选一）。 */
export type SshSecretSubmit = {
  /** 口令 / 密码明文（仅秘密输入类使用） */
  secret?: string;
  /** 确认类应答 */
  confirmed?: boolean;
  /** 是否取消 */
  cancelled?: boolean;
};
