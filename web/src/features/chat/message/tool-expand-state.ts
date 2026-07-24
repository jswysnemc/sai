import { useCallback, useState } from "react";

/** 用户显式展开过的 id */
const expandedIds = new Set<string>();
/** 用户显式收起过的 id（优先于默认展开） */
const collapsedIds = new Set<string>();

/**
 * 判断工具是否处于用户记忆的展开态。
 *
 * @param toolId 工具生命周期 id
 * @returns 是否已记录为展开
 */
export function isToolExpanded(toolId: string): boolean {
  return expandedIds.has(toolId) && !collapsedIds.has(toolId);
}

/**
 * 组内是否有用户已展开的工具。
 *
 * @param toolIds 组内工具 id
 * @returns 是否应默认展开组
 */
export function groupHasExpandedTool(toolIds: readonly string[]): boolean {
  return toolIds.some((id) => isToolExpanded(id));
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
 * 读写会话级展开状态，避免流式更新/分组重挂载后自动收缩。
 *
 * @param id 工具或工具组稳定标识
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
