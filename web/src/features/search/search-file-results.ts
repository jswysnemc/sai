import type { FileNode } from "../../api/contracts";

/**
 * 在当前工作区文件树中按文件名优先匹配路径，限制结果数量。
 * @param nodes 服务端返回的文件树
 * @param query 文件名或路径关键词
 * @param limit 最多返回多少个文件
 * @returns 去重、排序后的文件节点
 */
export function searchFileResults(nodes: FileNode[], query: string, limit = 60): FileNode[] {
  const normalized = query.trim().toLocaleLowerCase();
  const matches = new Map<string, FileNode>();
  const pending = [...nodes];
  // 1. 展开目录，只将实际文件加入候选结果
  while (pending.length) {
    const node = pending.pop()!;
    if (node.kind === "directory") pending.push(...node.children);
    else if (!normalized || node.path.toLocaleLowerCase().includes(normalized)) matches.set(node.path, node);
  }
  /**
   * 文件名的完整匹配优先于前缀、子串和父目录匹配。
   * @param node 待排序的文件节点
   * @returns 越小越优先的匹配等级
   */
  const rank = (node: FileNode): number => {
    const name = node.name.toLocaleLowerCase();
    if (name === normalized) return 0;
    if (name.startsWith(normalized)) return 1;
    if (name.includes(normalized)) return 2;
    return 3;
  };
  // 2. 稳定排序并截取可展示范围，避免长列表拖慢命令菜单
  return [...matches.values()].sort((left, right) =>
    rank(left) - rank(right)
    || left.path.split("/").length - right.path.split("/").length
    || left.path.localeCompare(right.path)
  ).slice(0, Math.max(0, limit));
}
