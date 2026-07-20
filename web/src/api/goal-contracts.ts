export type GoalStatus = "active" | "paused" | "blocked" | "usage_limited" | "budget_limited" | "complete";

export type GoalUpdateEntry = {
  at: string;
  kind: string;
  message: string;
  status?: string | null;
  tokens_used?: number | null;
};

export type Goal = {
  id: string;
  objective: string;
  status: GoalStatus;
  token_budget?: number | null;
  tokens_used: number;
  time_used_seconds: number;
  created_at: string;
  updated_at: string;
  updates?: GoalUpdateEntry[];
};

export type GoalResponse = {
  goal?: Goal | null;
};

export type GoalUpdateRequest = {
  status?: GoalStatus;
  objective?: string;
  token_budget?: number | null;
  note?: string;
};
