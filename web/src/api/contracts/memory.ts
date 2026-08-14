/** 记忆条目的类型；决定注入措辞与是否需要写明理由。 */
export type MemoryType = "user" | "feedback" | "project" | "reference";

/** 记忆的作用域；项目记忆仅在对应工作区可见。 */
export type MemoryScope = "global" | "project";

/** 列表里的一条记忆摘要，不含正文。 */
export type MemorySummary = {
  name: string;
  description: string;
  type: MemoryType;
  scope: MemoryScope;
};

/** 记忆列表响应。 */
export type MemoryListResult = {
  ok?: boolean;
  count?: number;
  entries: MemorySummary[];
};

/** 一条记忆的完整内容。 */
export type MemoryDetail = {
  found: boolean;
  name: string;
  description?: string;
  type?: MemoryType;
  scope?: MemoryScope;
  content?: string;
  /** 正文里用 [[标识]] 引用的其它记忆 */
  links?: string[];
};

/** 写入一条记忆的请求体。 */
export type MemoryWriteRequest = {
  name: string;
  description: string;
  content: string;
  memory_type: MemoryType;
  /** 为真时写入全局作用域，否则落在当前工作区 */
  global: boolean;
};

/** 记忆库状态汇总。 */
export type MemoryStats = {
  ok?: boolean;
  notes_dir?: string;
  memories?: number;
  project_memories?: number;
  global_memories?: number;
  evicted_turns?: number;
  storage?: { mode?: string };
};

/** 被压缩清出上下文的一条轮次。 */
export type EvictedTurnHit = {
  id: number;
  timestamp: string;
  role: string;
  score: number;
  snippet: string;
};

/** 逐出轮次的检索结果。 */
export type EvictedSearchResult = {
  ok?: boolean;
  query?: string;
  results: EvictedTurnHit[];
};
