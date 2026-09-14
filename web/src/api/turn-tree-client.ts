import { apiRequest } from "./api-request";
import type { SessionTurnTree, SessionTurnTreeIndex, TurnTreeNode } from "./turn-tree-contracts";

/**
 * 【会话分支】【界面结构】迭代连接扁平节点，保留界面已有的分支操作接口。
 * @param index 服务端返回的全部节点与活动位置
 * @returns 按序排列的分支树
 */
export function treeFromIndex(index: SessionTurnTreeIndex): SessionTurnTree {
  const nodes: TurnTreeNode[] = index.nodes.map((node) => ({ ...node, children: [] }));
  nodes.sort((left, right) => left.seq - right.seq);
  const byId = new Map(nodes.map((node) => [node.turn_id, node]));
  const roots: TurnTreeNode[] = [];
  for (const node of nodes) {
    const parent = node.parent_turn_id ? byId.get(node.parent_turn_id) : undefined;
    if (parent) parent.children.push(node);
    else roots.push(node);
  }
  return {
    roots,
    active_leaf_id: index.active_leaf_id,
    total_turns: index.total_turns,
    branch_points: index.branch_points,
  };
}

/**
 * 【会话分支】【读取】获取扁平索引，并兼容升级前的嵌套响应。
 * @param sessionId 会话标识
 * @returns 界面使用的完整分支树
 */
export async function loadTurnTree(sessionId: string): Promise<SessionTurnTree> {
  const data = await apiRequest<SessionTurnTreeIndex | SessionTurnTree>(`/api/sessions/${encodeURIComponent(sessionId)}/turn-tree`);
  return "nodes" in data ? treeFromIndex(data) : data;
}
