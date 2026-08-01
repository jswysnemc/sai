import type { SessionTurnTree, TurnTreeNode } from "../../../api/turn-tree-contracts";

/** 压平后的树行，供面板逐行渲染。 */
export type TurnTreeRow = {
  node: TurnTreeNode;
  /** 缩进层级，从 0 开始 */
  depth: number;
  /** 是否为同级最后一个，决定用 └ 还是 ├ */
  isLast: boolean;
  /** 各祖先层级是否还有后续兄弟，决定是否画竖线 */
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
  const rootCount = tree.roots.length;
  tree.roots.forEach((root, index) => {
    pushNode(root, 0, index + 1 === rootCount, [], activePath, tree.active_leaf_id, rows);
  });
  return rows;
}

/**
 * 递归写入节点及其后代。
 *
 * @param node 当前节点
 * @param depth 当前缩进层级
 * @param isLast 是否为同级最后一个
 * @param ancestorBars 各祖先层级是否还有后续兄弟
 * @param activePath 活动分支上的轮次集合
 * @param activeLeafId 当前所在轮次
 * @param rows 输出行
 * @returns 无返回值
 */
function pushNode(
  node: TurnTreeNode,
  depth: number,
  isLast: boolean,
  ancestorBars: boolean[],
  activePath: Set<string>,
  activeLeafId: string | null,
  rows: TurnTreeRow[]
): void {
  rows.push({
    node,
    depth,
    isLast,
    ancestorBars,
    isActive: node.turn_id === activeLeafId,
    onActivePath: activePath.has(node.turn_id)
  });
  const childBars = depth === 0 ? [] : [...ancestorBars, !isLast];
  node.children.forEach((child, index) => {
    pushNode(
      child,
      depth + 1,
      index + 1 === node.children.length,
      childBars,
      activePath,
      activeLeafId,
      rows
    );
  });
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
  const parents = new Map<string, string | null>();
  const walk = (node: TurnTreeNode) => {
    parents.set(node.turn_id, node.parent_turn_id);
    node.children.forEach(walk);
  };
  tree.roots.forEach(walk);

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
  let result: { siblings: TurnTreeNode[]; index: number } | null = null;
  const visit = (nodes: TurnTreeNode[]) => {
    const index = nodes.findIndex((node) => node.turn_id === turnId);
    // 只有真正存在分叉时才需要版本切换
    if (index >= 0 && nodes.length > 1) {
      result = { siblings: nodes, index };
      return;
    }
    nodes.forEach((node) => visit(node.children));
  };
  visit(tree.roots);
  return result;
}
