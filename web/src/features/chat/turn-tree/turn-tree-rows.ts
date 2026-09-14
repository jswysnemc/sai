import type { SessionTurnTree, TurnTreeNode } from "../../../api/turn-tree-contracts";
import { indexTurnTree } from "./turn-tree-index";

const MAX_ANCESTOR_BARS = 6;

/** 压平后的树行，供面板逐行渲染。 */
export type TurnTreeRow = {
  node: TurnTreeNode;
  /** 缩进层级，从 0 开始 */
  depth: number;
  /** 是否为同级最后一个，决定用 └ 还是 ├ */
  isLast: boolean;
  /** 最近六层祖先是否还有后续兄弟，决定是否画竖线 */
  ancestorBars: boolean[];
  /** 是否为当前所在轮次 */
  isActive: boolean;
  /** 是否位于活动分支的路径上 */
  onActivePath: boolean;
};

/**
 * 把会话树压平成可逐行渲染的列表。
 *
 * 树在页面上仍以行的形式呈现，父子关系靠缩进与连接线表达。
 *
 * @param tree 会话分支树
 * @returns 自上而下的展示行
 */
export function flattenTurnTree(tree: SessionTurnTree): TurnTreeRow[] {
  const activePath = new Set(collectActivePath(tree));
  const rows: TurnTreeRow[] = [];
  const pending = tree.roots.map((node, index) => ({
    node, depth: 0, isLast: index + 1 === tree.roots.length, ancestorBars: [] as boolean[],
  })).reverse();
  const seen = new Set<string>();
  while (pending.length > 0) {
    const item = pending.pop()!;
    if (seen.has(item.node.turn_id)) continue;
    seen.add(item.node.turn_id);
    rows.push({ ...item, isActive: item.node.turn_id === tree.active_leaf_id, onActivePath: activePath.has(item.node.turn_id) });
    // 1. 【会话分支】【缩进上限】面板只展示最近六层，避免长链为每轮复制全部祖先
    const childBars = item.depth === 0 ? [] : [...item.ancestorBars, !item.isLast].slice(-MAX_ANCESTOR_BARS);
    for (let index = item.node.children.length - 1; index >= 0; index--) {
      pending.push({
        node: item.node.children[index], depth: item.depth + 1,
        isLast: index + 1 === item.node.children.length, ancestorBars: childBars,
      });
    }
  }
  return rows;
}

/**
 * 收集从根到当前所在轮次的路径。
 *
 * 路径上的节点在面板里高亮，用于表达"当前对话经过了哪些轮次"。
 *
 * @param tree 会话分支树
 * @returns 路径上的轮次标识
 */
export function collectActivePath(tree: SessionTurnTree): string[] {
  if (!tree.active_leaf_id) return [];
  const { parents } = indexTurnTree(tree);

  const path: string[] = [];
  const seen = new Set<string>();
  let cursor: string | null | undefined = tree.active_leaf_id;
  // 自叶向根回溯；seen 兜住异常数据形成的环
  while (cursor && !seen.has(cursor)) {
    seen.add(cursor);
    path.push(cursor);
    cursor = parents.get(cursor) ?? null;
  }
  return path.reverse();
}

/**
 * 找出某个轮次所在的兄弟组，用于「第 n / 共 m 个版本」切换。
 *
 * @param tree 会话分支树
 * @param turnId 目标轮次
 * @returns 同级轮次列表与当前所处位置；无分叉时返回 null
 */
export function findSiblingBranches(
  tree: SessionTurnTree,
  turnId: string
): { siblings: TurnTreeNode[]; index: number } | null {
  return indexTurnTree(tree).siblings.get(turnId) ?? null;
}

/**
 * 找出分支中序号最大的叶节点。
 *
 * 版本切换面向完整分支，选择兄弟根节点会把该分支已有的后续对话隐藏。
 *
 * @param branch 目标分支根节点
 * @returns 应恢复的叶节点标识
 */
export function preferredBranchLeafId(branch: TurnTreeNode): string {
  const pending = [branch];
  const seen = new Set<string>();
  let preferred: TurnTreeNode | undefined;
  while (pending.length > 0) {
    const node = pending.pop()!;
    if (seen.has(node.turn_id)) continue;
    seen.add(node.turn_id);
    if (node.children.length === 0 && (!preferred || node.seq > preferred.seq)) preferred = node;
    for (let index = node.children.length - 1; index >= 0; index--) pending.push(node.children[index]);
  }
  return (preferred ?? branch).turn_id;
}
