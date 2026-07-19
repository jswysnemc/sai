import { useEffect, useMemo, useState } from "react";

export type ChangeSelectionModifier = {
  toggle: boolean;
  range: boolean;
};

/**
 * 根据点击修饰键更新 Source Control 文件选择。
 *
 * @param current 当前选择路径
 * @param orderedPaths 当前可见路径顺序
 * @param anchorPath 上次选择锚点
 * @param path 本次点击路径
 * @param modifier Ctrl/Cmd 切换和 Shift 范围状态
 * @returns 新选择路径
 */
export function updateChangeSelection(
  current: ReadonlySet<string>,
  orderedPaths: string[],
  anchorPath: string | null,
  path: string,
  modifier: ChangeSelectionModifier
): Set<string> {
  if (modifier.range && anchorPath) {
    const anchorIndex = orderedPaths.indexOf(anchorPath);
    const pathIndex = orderedPaths.indexOf(path);
    if (anchorIndex >= 0 && pathIndex >= 0) {
      const [start, end] = anchorIndex <= pathIndex
        ? [anchorIndex, pathIndex]
        : [pathIndex, anchorIndex];
      const range = orderedPaths.slice(start, end + 1);
      return modifier.toggle ? new Set([...current, ...range]) : new Set(range);
    }
  }
  if (modifier.toggle) {
    const next = new Set(current);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    return next;
  }
  return new Set([path]);
}

/**
 * 管理单仓库 Source Control 文件多选和范围锚点。
 *
 * @param orderedPaths 当前可见文件路径顺序
 * @returns 选择集合、更新方法和清空方法
 */
export function useChangeSelection(orderedPaths: string[]) {
  const [selectedPaths, setSelectedPaths] = useState<Set<string>>(() => new Set());
  const [anchorPath, setAnchorPath] = useState<string | null>(null);
  const available = useMemo(() => new Set(orderedPaths), [orderedPaths]);

  useEffect(() => {
    setSelectedPaths((current) => new Set([...current].filter((path) => available.has(path))));
    setAnchorPath((current) => current && available.has(current) ? current : null);
  }, [available]);

  /** 按鼠标修饰键选择文件。 */
  const select = (path: string, modifier: ChangeSelectionModifier) => {
    setSelectedPaths((current) => updateChangeSelection(current, orderedPaths, anchorPath, path, modifier));
    setAnchorPath(path);
  };

  /** 右键未选中文件时将其设为唯一选择。 */
  const selectForContext = (path: string) => {
    setSelectedPaths((current) => current.has(path) ? current : new Set([path]));
    setAnchorPath(path);
  };

  return {
    selectedPaths,
    select,
    selectForContext,
    clear: () => {
      setSelectedPaths(new Set());
      setAnchorPath(null);
    }
  };
}
