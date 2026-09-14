import { describe, expect, it } from "vitest";
import type { SessionTurnTree, TurnTreeNode } from "../../../api/turn-tree-contracts";
import { collectActivePath, findSiblingBranches, flattenTurnTree, preferredBranchLeafId } from "./turn-tree-rows";

/**
 * 【会话分支】【深度样本】迭代构造长对话，测试本身不依赖递归。
 * @param count 轮次数量
 * @returns 线性分支树
 */
function longConversation(count: number): SessionTurnTree {
  const roots: TurnTreeNode[] = [];
  let children = roots;
  for (let index = 1; index <= count; index++) {
    const node: TurnTreeNode = {
      turn_id: `turn-${index}`, parent_turn_id: index === 1 ? null : `turn-${index - 1}`,
      seq: index, user_summary: "question", assistant_summary: "answer", status: "completed", timestamp: "", children: [],
    };
    children.push(node);
    children = node.children;
  }
  return { roots, active_leaf_id: `turn-${count}`, total_turns: count, branch_points: 0 };
}

describe("long conversation branches", () => {
  it("finds the active path without recursive stack growth", () => {
    expect(collectActivePath(longConversation(5000))).toHaveLength(5000);
  });
  it("finds sibling versions without recursive scans", () => {
    expect(findSiblingBranches(longConversation(5000), "turn-5000")).toBeNull();
  });
  it("finds the preferred leaf without recursive stack growth", () => {
    expect(preferredBranchLeafId(longConversation(5000).roots[0])).toBe("turn-5000");
  });
  it("flattens the complete tree without recursive stack growth", () => {
    const rows = flattenTurnTree(longConversation(5000));
    expect(rows).toHaveLength(5000);
    expect(rows.at(-1)?.node.turn_id).toBe("turn-5000");
  });
  it("keeps only the six visible ancestor bars for each turn", () => {
    const rows = flattenTurnTree(longConversation(5000));
    expect(rows.at(-1)?.depth).toBe(4999);
    expect(Math.max(...rows.map((row) => row.ancestorBars.length))).toBeLessThanOrEqual(6);
  });
});
