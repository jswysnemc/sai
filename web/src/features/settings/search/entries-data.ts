import type { SettingsSearchEntry } from "./settings-search-types";

/** 数据管理与用量筛选字段，不读取条目内容。 */
export const DATA_SEARCH_ENTRIES: SettingsSearchEntry[] = [
  ...[["search", "Search sessions", "搜索会话"], ["workspace", "Workspace filter", "筛选工作区"], ["size", "Minimum session size", "会话最小大小"]].map(([key, labelEn, labelZh]) => ({ section: "session-data" as const, anchor: `session-data.${key}`, labelEn, labelZh, keywords: [key, "session", "会话"] })),
  ...[["range", "Usage time range", "用量时间范围"], ["source", "Usage source", "用量来源"], ["status", "Usage status", "用量状态"], ["provider", "Usage provider", "用量供应商"], ["model", "Usage model", "用量模型"]].map(([key, labelEn, labelZh]) => ({ section: "usage" as const, subview: "overview", anchor: `usage.${key}`, labelEn, labelZh, keywords: [key, "usage", "用量"] }))
];
