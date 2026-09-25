import type { FileNode } from "../../api/contracts";

/**
 * 在文件树中查找指定路径节点。
 *
 * @param nodes 待检索的文件树节点
 * @param path 目标相对路径
 * @returns 匹配节点，未找到时返回 null
 */
export function findFileNode(nodes: FileNode[], path: string | null): FileNode | null {
  if (!path) return null;
  for (const node of nodes) {
    if (node.path === path) return node;
    const child = findFileNode(node.children, path);
    if (child) return child;
  }
  return null;
}

/**
 * 按名称过滤文件树，并保留命中节点的父目录。
 *
 * 关键词不含 `/` 时只匹配文件名，避免输入目录名就把该目录下的全部子孙都算作命中。
 * 关键词含 `/` 时按相对路径匹配，方便直接找 `src/agent/foo` 这类位置。
 *
 * @param nodes 原始文件树节点
 * @param query 搜索关键词
 * @returns 过滤后的文件树节点
 */
export function filterFileNodes(nodes: FileNode[], query: string): FileNode[] {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return nodes;
  const matchPath = normalized.includes("/");
  return nodes.flatMap((node) => {
    const children = filterFileNodes(node.children, normalized);
    const haystack = matchPath ? node.path : node.name;
    const matched = haystack.toLocaleLowerCase().includes(normalized);
    if (!matched && children.length === 0) return [];
    return [{ ...node, children }];
  });
}

export type FlatTreeRow = {
  node: FileNode;
  depth: number;
};

/**
 * 收集路径上的祖先目录，从根到父目录。
 *
 * @param path 文件或目录相对路径
 * @returns 祖先目录路径
 */
export function ancestorPaths(path: string): string[] {
  const ancestors: string[] = [];
  let parent = parentFilePath(path);
  while (parent) {
    ancestors.push(parent);
    parent = parentFilePath(parent);
  }
  ancestors.reverse();
  return ancestors;
}

/**
 * 把目标文件的祖先目录并入展开集合。
 *
 * @param paths 当前已展开目录
 * @param target 需要露出来的路径；为空时原样返回
 * @returns 已包含祖先的集合；无需变更时返回原集合
 */
export function withAncestors(paths: ReadonlySet<string>, target: string | null): ReadonlySet<string> {
  if (!target) return paths;
  const ancestors = ancestorPaths(target);
  if (ancestors.every((path) => paths.has(path))) return paths;
  const next = new Set(paths);
  for (const path of ancestors) next.add(path);
  return next;
}

/**
 * 收集树里的全部目录路径。
 *
 * @param nodes 文件树节点
 * @param paths 写入目标
 * @returns 目录路径集合
 */
export function directoryPaths(nodes: FileNode[], paths = new Set<string>()): Set<string> {
  for (const node of nodes) {
    if (node.kind !== "directory") continue;
    paths.add(node.path);
    directoryPaths(node.children, paths);
  }
  return paths;
}

/**
 * 把展开后的树压成从上到下的行，折叠目录的子孙不会进入结果。
 *
 * @param nodes 文件树节点
 * @param expanded 已展开的目录路径
 * @param depth 当前深度
 * @param rows 写入目标
 * @returns 可见行
 */
export function flattenVisibleNodes(nodes: FileNode[], expanded: ReadonlySet<string>, depth = 0, rows: FlatTreeRow[] = []): FlatTreeRow[] {
  for (const node of nodes) {
    rows.push({ node, depth });
    if (node.kind === "directory" && expanded.has(node.path)) flattenVisibleNodes(node.children, expanded, depth + 1, rows);
  }
  return rows;
}

/**
 * 用按需读到的子目录补上深度截断后的空目录。
 *
 * 服务端已经带回子节点时以服务端为准；只有子节点为空时才使用补丁。
 *
 * @param nodes 服务端文件树
 * @param lazy 目录路径到子节点的补丁
 * @returns 合并后的文件树；没有补丁命中时返回原数组
 */
export function applyLazyChildren(nodes: FileNode[], lazy: ReadonlyMap<string, FileNode[]>): FileNode[] {
  let changed = false;
  const next = nodes.map((node) => {
    if (node.kind !== "directory") return node;
    const base = node.children.length > 0 ? node.children : lazy.get(node.path) ?? node.children;
    const children = base.length === 0 ? base : applyLazyChildren(base, lazy);
    if (children === node.children) return node;
    changed = true;
    return { ...node, children };
  });
  return changed ? next : nodes;
}

/**
 * 找到通往目标路径时、子节点还没加载的目录。
 *
 * @param nodes 当前文件树
 * @param targetPath 希望定位的相对路径
 * @returns 需要继续读取的目录；目标已在树中或路径不存在时返回 null
 */
export function truncatedDirectoryFor(nodes: FileNode[], targetPath: string): string | null {
  if (findFileNode(nodes, targetPath)) return null;
  let cursor = nodes;
  for (const path of ancestorPaths(targetPath)) {
    const node = cursor.find((item) => item.path === path);
    if (!node || node.kind !== "directory") return null;
    if (node.children.length === 0) return node.path;
    cursor = node.children;
  }
  return null;
}

/**
 * 计算虚拟列表当前要挂载的行区间。
 *
 * @param count 可见行总数
 * @param scrollTop 滚动容器的 scrollTop
 * @param viewport 滚动容器可视高度；未知时先按大约四十行估算
 * @param rowHeight 单行高度
 * @param overscan 上下额外挂载的行数
 * @returns 半开区间 [start, end)
 */
export function visibleRowRange(count: number, scrollTop: number, viewport: number, rowHeight: number, overscan = 8): { start: number; end: number } {
  if (count <= 0 || rowHeight <= 0) return { start: 0, end: 0 };
  const span = viewport > 0 ? viewport : rowHeight * 40;
  const top = Math.max(0, scrollTop);
  const start = Math.max(0, Math.floor(top / rowHeight) - overscan);
  const end = Math.min(count, Math.ceil((top + span) / rowHeight) + overscan);
  return { start, end: Math.max(start, end) };
}

/**
 * 返回相对路径的父目录。
 *
 * @param path 文件或目录相对路径
 * @returns 父目录相对路径，根目录返回空字符串
 */
export function parentFilePath(path: string): string {
  const index = path.lastIndexOf("/");
  return index < 0 ? "" : path.slice(0, index);
}
