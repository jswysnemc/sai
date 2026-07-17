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
 * 按名称或路径过滤文件树，并保留命中节点的父目录。
 *
 * @param nodes 原始文件树节点
 * @param query 搜索关键词
 * @returns 过滤后的文件树节点
 */
export function filterFileNodes(nodes: FileNode[], query: string): FileNode[] {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return nodes;
  return nodes.flatMap((node) => {
    const children = filterFileNodes(node.children, normalized);
    const matched = node.name.toLocaleLowerCase().includes(normalized) || node.path.toLocaleLowerCase().includes(normalized);
    if (!matched && children.length === 0) return [];
    return [{ ...node, children }];
  });
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
