export type UsageRange = "today" | "1d" | "7d" | "30d" | "90d" | "all";

export type UsageRecord = {
  id: string;
  created_at: number;
  completed_at: number;
  duration_ms: number;
  source: string;
  operation: string;
  provider_id: string;
  provider_name: string;
  model: string;
  status: string;
  usage_source: string;
  input_tokens?: number | null;
  output_tokens?: number | null;
  total_tokens?: number | null;
  session_id?: string | null;
  error_kind?: string | null;
};

export type UsageSummary = {
  total_requests: number;
  successful_requests: number;
  failed_requests: number;
  missing_usage_requests: number;
  provider_reported_requests: number;
  total_tokens: number;
  input_tokens: number;
  output_tokens: number;
  average_duration_ms?: number | null;
};

export type UsageTrendPoint = {
  date: string;
  label: string;
  requests: number;
  total_tokens: number;
  input_tokens: number;
  output_tokens: number;
};

export type UsageGroupStats = {
  id: string;
  label: string;
  provider_id?: string | null;
  provider_name?: string | null;
  model?: string | null;
  request_count: number;
  success_count: number;
  total_tokens: number;
  input_tokens: number;
  output_tokens: number;
  average_duration_ms?: number | null;
  last_used_at?: number | null;
};

export type UsageStatsResponse = {
  summary: UsageSummary;
  trend: UsageTrendPoint[];
  logs: UsageRecord[];
  provider_stats: UsageGroupStats[];
  model_stats: UsageGroupStats[];
  total_logs: number;
  skipped_records: number;
};

export type UsageStatsQuery = {
  range?: UsageRange | string;
  source?: string;
  status?: string;
  provider_search?: string;
  model_search?: string;
  limit?: number;
  offset?: number;
};
