import type { SessionTurnTree, TurnTreeNode } from "../../../api/turn-tree-contracts";

type SiblingGroup = { siblings: TurnTreeNode[]; index: number };
type TreeIndex = { parents: Map<string, string | null>; siblings: Map<string, SiblingGroup> };
const indexes = new WeakMap<SessionTurnTree, TreeIndex>();

/**
 * 【会话分支】【查找索引】迭代索引父关系和分叉组，避免每条历史消息重复遍历整棵树。
 * @param tree 当前不可变查询结果
 * @returns 供路径与版本切换复用的索引
 */
export function indexTurnTree(tree: SessionTurnTree): TreeIndex {
  const cached = indexes.get(tree);
  if (cached) return cached;
  const parents = new Map<string, string | null>();
  const siblings = new Map<string, SiblingGroup>();
  const pending = [tree.roots];
  while (pending.length > 0) {
    const group = pending.pop()!;
    group.forEach((node, index) => {
      if (parents.has(node.turn_id)) return;
      parents.set(node.turn_id, node.parent_turn_id);
      if (group.length > 1) siblings.set(node.turn_id, { siblings: group, index });
      if (node.children.length > 0) pending.push(node.children);
    });
  }
  const result = { parents, siblings };
  indexes.set(tree, result);
  return result;
}
