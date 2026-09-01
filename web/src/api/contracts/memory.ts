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
  /** 索引里的提示行；编辑保存时必须原样带回，否则会被重置成摘要 */
  hook?: string;
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
  /** 索引行里的一句话提示；留空沿用摘要 */
  hook?: string;
  /** 工作区标识；省略时用当前活动工作区 */
  workspace?: string;
};

/** 写入一条记忆的结果。 */
export type MemoryWriteResult = {
  ok?: boolean;
  name: string;
  scope: MemoryScope;
  /** 为真表示覆盖了同名条目 */
  updated: boolean;
  /** 正文里引用的其它记忆 */
  links: string[];
  /** 需要写明理由的类型缺了 Why/How to apply 时的软提示 */
  note?: string;
};

/** 每轮实际注入给模型的记忆索引。 */
export type MemoryIndexResult = {
  ok?: boolean;
  /** 为空表示当前没有任何记忆，本轮不会注入索引 */
  injected: boolean;
  text: string;
};

/** 记忆接口共用的工作区参数。 */
export type MemoryQuery = {
  workspace?: string;
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
