/**
 * 文件树多选的纯状态计算：单击替换、Ctrl/⌘ 增减、Shift 区间。
 */

/** 多选状态：已选路径集合与区间选择的锚点。 */
export type TreeSelection = {
  paths: ReadonlySet<string>;
  anchor: string | null;
};

/** 点击时的修饰键。 */
export type SelectModifiers = {
  toggle: boolean;
  range: boolean;
};

export const EMPTY_TREE_SELECTION: TreeSelection = { paths: new Set(), anchor: null };

/**
 * 按点击和修饰键计算下一个多选状态。
 *
 * 步骤:
 * 1. Shift：从锚点到目标按可见顺序整段选中；按住 Ctrl/⌘ 时并入已有选择
 * 2. Ctrl/⌘：增减目标，锚点移到目标
 * 3. 无修饰键：只选中目标
 *
 * @param current 当前多选状态
 * @param path 被点击的路径
 * @param order 当前可见行的路径顺序
 * @param modifiers 修饰键
 * @returns 下一个多选状态
 */
export function nextTreeSelection(current: TreeSelection, path: string, order: string[], modifiers: SelectModifiers): TreeSelection {
  if (modifiers.range && current.anchor !== null) {
    const from = order.indexOf(current.anchor);
    const to = order.indexOf(path);
    if (from >= 0 && to >= 0) {
      const [start, end] = from <= to ? [from, to] : [to, from];
      const span = order.slice(start, end + 1);
      const paths = new Set(modifiers.toggle ? current.paths : []);
      span.forEach((item) => paths.add(item));
      return { paths, anchor: current.anchor };
    }
  }
  if (modifiers.toggle) {
    const paths = new Set(current.paths);
    if (paths.has(path)) paths.delete(path);
    else paths.add(path);
    return { paths, anchor: path };
  }
  return { paths: new Set([path]), anchor: path };
}

/**
 * 右键时决定操作对象：点在已选条目上沿用整个选择，否则改为只选该条目。
 *
 * @param current 当前多选状态
 * @param path 右键所在路径
 * @returns 右键后的多选状态
 */
export function contextTreeSelection(current: TreeSelection, path: string): TreeSelection {
  return current.paths.has(path) ? current : { paths: new Set([path]), anchor: path };
}

/**
 * 去掉被已选祖先目录覆盖的路径，批量删除时不重复处理子项。
 *
 * @param paths 已选路径
 * @returns 只保留最外层的路径，按字典序排列
 */
export function outermostPaths(paths: Iterable<string>): string[] {
  const sorted = [...new Set(paths)].sort();
  return sorted.filter((path) => !sorted.some((other) => other !== path && path.startsWith(`${other}/`)));
}

/**
 * 剔除已不在树中的路径（删除、改名或切换工作区后）。
 *
 * @param current 当前多选状态
 * @param exists 判断路径是否仍存在
 * @returns 清理后的多选状态；无变化时返回原对象
 */
export function pruneTreeSelection(current: TreeSelection, exists: (path: string) => boolean): TreeSelection {
  const kept = [...current.paths].filter(exists);
  if (kept.length === current.paths.size) return current;
  return { paths: new Set(kept), anchor: current.anchor && exists(current.anchor) ? current.anchor : null };
}
