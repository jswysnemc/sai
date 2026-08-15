import { useCallback, useState } from "react";

/** 用户显式展开过的 id */
const expandedIds = new Set<string>();
/** 用户显式收起过的 id（优先于默认展开） */
const collapsedIds = new Set<string>();

/**
 * 清空跨会话残留的展开记忆，避免 SPA 长生命周期下 Set 无限增长。
 *
 * @returns 无
 */
export function clearToolExpandState(): void {
  expandedIds.clear();
  collapsedIds.clear();
}

/**
 * 解析会话级展开偏好：用户操作优先于默认值。
 *
 * @param id 稳定标识
 * @param initial 无用户记忆时的默认值
 * @returns 是否展开
 */
function resolveExpanded(id: string, initial: boolean): boolean {
  if (collapsedIds.has(id)) return false;
  if (expandedIds.has(id)) return true;
  return initial;
}

/**
 * 读写会话级展开状态，避免流式更新导致重挂载后自动收缩。
 *
 * @param id 工具稳定标识
 * @param initial 首次且无记忆时的默认值
 * @returns 展开状态与切换函数
 */
export function usePersistedExpand(
  id: string,
  initial = false
): [boolean, (next: boolean | ((value: boolean) => boolean)) => void] {
  const [expanded, setExpandedState] = useState(() => resolveExpanded(id, initial));

  const setExpanded = useCallback(
    (next: boolean | ((value: boolean) => boolean)) => {
      setExpandedState((current) => {
        const value = typeof next === "function" ? next(current) : next;
        // 1. 记录用户显式意图，覆盖默认展开/收起
        if (value) {
          expandedIds.add(id);
          collapsedIds.delete(id);
        } else {
          collapsedIds.add(id);
          expandedIds.delete(id);
        }
        return value;
      });
    },
    [id]
  );

  return [expanded, setExpanded];
}
