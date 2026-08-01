/** 会话树中的单个轮次节点。 */
export type TurnTreeNode = {
  turn_id: string;
  parent_turn_id: string | null;
  seq: number;
  /** 用户输入摘要，单行 */
  user_summary: string;
  /** 助手回复摘要，单行 */
  assistant_summary: string;
  status: string;
  timestamp: string;
  children: TurnTreeNode[];
};

/** 会话的完整分支树。 */
export type SessionTurnTree = {
  roots: TurnTreeNode[];
  /** 当前所在轮次；会话为空时为 null */
  active_leaf_id: string | null;
  total_turns: number;
  /** 分叉点数量：拥有多个子节点的轮次 */
  branch_points: number;
};

/** 分支操作后的活动叶子。 */
export type BranchSwitchResult = {
  active_leaf_id: string | null;
};
