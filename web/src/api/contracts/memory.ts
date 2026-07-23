export type MemoryEntry = {
  id: number;
  kind: "fact" | "episode";
  content: string;
  source: string;
  status: string;
  strength?: number;
  confidence?: number;
  recall_count?: number;
  created_at: string;
  updated_at: string;
  has_markdown?: boolean;
  markdown_path?: string;
};

export type MemoryStorageStats = {
  mode?: string;
  markdown_facts?: number;
  markdown_episodes?: number;
  fts?: {
    facts?: number;
    facts_trigram?: number;
    episodes?: number;
    episodes_trigram?: number;
    ready?: boolean;
  };
};

export type MemoryStats = {
  ok?: boolean;
  data_db?: string;
  state_db?: string;
  files_dir?: string;
  skills_dir?: string;
  facts?: number;
  episodes?: number;
  unprocessed_pending_events?: number;
  total_pending_events?: number;
  skill_records?: number;
  skill_dirs?: number;
  evicted_turns?: number;
  storage?: MemoryStorageStats;
};

export type MemorySearchHit = {
  id: number;
  content: string;
  score: number;
  timestamp: string;
  source: string;
};

export type MemorySearchResult = {
  ok?: boolean;
  query?: string;
  facts?: MemorySearchHit[];
  episodes?: MemorySearchHit[];
};
