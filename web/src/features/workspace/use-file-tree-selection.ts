import { useCallback, useEffect, useState } from "react";
import {
  contextTreeSelection,
  EMPTY_TREE_SELECTION,
  nextTreeSelection,
  pruneTreeSelection,
  type SelectModifiers,
  type TreeSelection
} from "./file-tree-selection";

/**
 * 【工作区】【文件树多选】管理多选状态，树结构变化后剔除已失效的路径。
 *
 * @param order 当前可见行的路径顺序
 * @param exists 判断路径是否仍在树中
 * @returns 多选状态与操作方法
 */
export function useFileTreeSelection(order: string[], exists: (path: string) => boolean) {
  const [selection, setSelection] = useState<TreeSelection>(EMPTY_TREE_SELECTION);

  // 1. 删除、改名或切换工作区后，去掉已不存在的路径
  useEffect(() => {
    setSelection((current) => pruneTreeSelection(current, exists));
  }, [exists]);

  /** 按点击和修饰键更新选择。 */
  const click = useCallback((path: string, modifiers: SelectModifiers) => {
    setSelection((current) => nextTreeSelection(current, path, order, modifiers));
  }, [order]);

  /**
   * 右键前确定操作对象并返回最新选择。
   *
   * @param path 右键所在路径
   * @returns 右键后的已选路径
   */
  const context = useCallback((path: string): string[] => {
    const next = contextTreeSelection(selection, path);
    setSelection(next);
    return [...next.paths];
  }, [selection]);

  /** 选中全部可见行。 */
  const selectAll = useCallback(() => {
    setSelection({ paths: new Set(order), anchor: order[0] ?? null });
  }, [order]);

  /** 清空选择。 */
  const clear = useCallback(() => setSelection(EMPTY_TREE_SELECTION), []);

  return { selection, click, context, selectAll, clear };
}
