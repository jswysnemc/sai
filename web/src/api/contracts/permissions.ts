export type RunMode = "plan" | "audited" | "auto_audit" | "yolo";

export type PermissionConfig = {
  default_mode: RunMode;
  tui_mode?: RunMode;
  cli_mode?: RunMode;
  auto_audit_provider_id?: string;
  auto_audit_model?: string;
};

export type SessionConfig = {
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
  | { decision: "allow"; source?: PermissionAllowSource }
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
