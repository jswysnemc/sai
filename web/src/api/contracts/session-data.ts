export type SessionDataItem = {
  name: string;
  kind: "file" | "directory" | "other";
  bytes: number;
  file_count: number;
};

export type SessionDataSummary = {
  id: string;
  title: string;
  created_at: string;
  updated_at: string;
  active: boolean;
  total_bytes: number;
  file_count: number;
  turn_count?: number | null;
  branch_points?: number | null;
  loaded_tool_count?: number | null;
  todo_count?: number | null;
  has_goal?: boolean | null;
  state_error?: string | null;
  items: SessionDataItem[];
};
