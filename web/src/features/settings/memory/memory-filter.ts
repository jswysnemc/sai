import type { MemoryScope, MemorySummary, MemoryType } from "../../../api/contracts";

/** 类型筛选；all 表示不过滤。 */
export type MemoryTypeFilter = MemoryType | "all";

/** 作用域筛选；all 表示不过滤。 */
export type MemoryScopeFilter = MemoryScope | "all";

/** 记忆列表的筛选条件。 */
export type MemoryFilter = {
  type: MemoryTypeFilter;
  scope: MemoryScopeFilter;
  query: string;
};

/** 空筛选条件：不过滤任何条目。 */
export const EMPTY_MEMORY_FILTER: MemoryFilter = {
  type: "all",
  scope: "all",
  query: ""
};

/**
 * 按类型、作用域与关键字筛选记忆。
 *
 * 关键字同时匹配标识与摘要：标识是 `[[关联]]` 的目标，摘要是索引里看到的
 * 那一行，漏掉任一边都会让人找不到记得清清楚楚的条目。
 *
 * @param entries 全部记忆条目
 * @param filter 筛选条件
 * @returns 命中条件的条目
 */
export function filterMemories(entries: MemorySummary[], filter: MemoryFilter): MemorySummary[] {
  const keyword = filter.query.trim().toLowerCase();
  return entries.filter((entry) => {
    if (filter.type !== "all" && entry.type !== filter.type) return false;
    if (filter.scope !== "all" && entry.scope !== filter.scope) return false;
    if (!keyword) return true;
    return (
      entry.name.toLowerCase().includes(keyword) ||
      entry.description.toLowerCase().includes(keyword)
    );
  });
}

/**
 * 统计各类型与作用域的条目数，供筛选栏显示角标。
 *
 * 计数只按单一维度统计，不叠加其它条件：叠加会让数字随筛选变化，
 * 反而看不出每个桶里到底有多少条。
 *
 * @param entries 全部记忆条目
 * @returns 类型与作用域的条数
 */
export function countMemories(entries: MemorySummary[]): {
  types: Record<MemoryTypeFilter, number>;
  scopes: Record<MemoryScopeFilter, number>;
} {
  const types: Record<MemoryTypeFilter, number> = {
    all: entries.length,
    user: 0,
    feedback: 0,
    project: 0,
    reference: 0
  };
  const scopes: Record<MemoryScopeFilter, number> = {
    all: entries.length,
    global: 0,
    project: 0
  };
  for (const entry of entries) {
    types[entry.type] += 1;
    scopes[entry.scope] += 1;
  }
  return { types, scopes };
}

/**
 * 判断正文是否缺需要说明理由的小标题。
 *
 * 与后端 `MemoryType::missing_rationale` 同一套规则：缺了理由，下一轮
 * 无从判断这条在新情境下还适不适用。
 *
 * @param type 条目类型
 * @param content 正文
 * @returns 缺失的小标题；无需理由或已写全时为空数组
 */
export function missingRationaleMarkers(type: MemoryType, content: string): string[] {
  if (type !== "feedback" && type !== "project") return [];
  return ["Why:", "How to apply:"].filter((marker) => !content.includes(marker));
}
