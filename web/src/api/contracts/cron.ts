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
