import { describe, expect, it } from "vitest";
import type { SessionTurnTree, TurnTreeNode } from "../../../api/turn-tree-contracts";
import { collectActivePath, findSiblingBranches, flattenTurnTree } from "./turn-tree-rows";

/** 构造测试节点。 */
function node(id: string, parent: string | null, children: TurnTreeNode[] = []): TurnTreeNode {
  return {
    turn_id: id,
    parent_turn_id: parent,
    seq: Number(id.replace(/\D/g, "")) || 0,
    user_summary: `问题 ${id}`,
    assistant_summary: `回答 ${id}`,
    status: "completed",
    timestamp: "",
    children
  };
}

/** 构造测试树。 */
function tree(roots: TurnTreeNode[], activeLeafId: string | null): SessionTurnTree {
  return { roots, active_leaf_id: activeLeafId, total_turns: 0, branch_points: 0 };
}

describe("flattenTurnTree", () => {
  it("按深度优先压平，并标注缩进层级", () => {
    const rows = flattenTurnTree(
      tree([node("t1", null, [node("t2", "t1", [node("t3", "t2")])])], "t3")
    );

    expect(rows.map((row) => row.node.turn_id)).toEqual(["t1", "t2", "t3"]);
    expect(rows.map((row) => row.depth)).toEqual([0, 1, 2]);
  });

  it("标记同级最后一项，用于选择连接线样式", () => {
    const rows = flattenTurnTree(
      tree([node("t1", null, [node("t2", "t1"), node("t3", "t1")])], "t3")
    );

    expect(rows[1].isLast).toBe(false);
    expect(rows[2].isLast).toBe(true);
  });

  it("只有当前所在轮次是 active，路径上的其余节点标记为 onActivePath", () => {
    const rows = flattenTurnTree(
      tree([node("t1", null, [node("t2", "t1"), node("t3", "t1")])], "t3")
    );

    const byId = new Map(rows.map((row) => [row.node.turn_id, row]));
    expect(byId.get("t3")?.isActive).toBe(true);
    expect(byId.get("t1")?.isActive).toBe(false);
    // t1 是 t3 的祖先，属于活动路径
    expect(byId.get("t1")?.onActivePath).toBe(true);
    // t2 是被切走的分支
    expect(byId.get("t2")?.onActivePath).toBe(false);
  });
});

describe("collectActivePath", () => {
  it("返回从根到当前轮次的顺序路径", () => {
    const result = collectActivePath(
      tree([node("t1", null, [node("t2", "t1", [node("t3", "t2")])])], "t3")
    );

    expect(result).toEqual(["t1", "t2", "t3"]);
  });

  it("分支路径不含兄弟节点", () => {
    const result = collectActivePath(
      tree([node("t1", null, [node("t2", "t1"), node("t3", "t1")])], "t3")
    );

    expect(result).toEqual(["t1", "t3"]);
  });

  it("没有活动轮次时返回空路径", () => {
    expect(collectActivePath(tree([node("t1", null)], null))).toEqual([]);
  });
});

describe("findSiblingBranches", () => {
  it("有分叉时返回同级列表与当前位置", () => {
    const result = findSiblingBranches(
      tree([node("t1", null, [node("t2", "t1"), node("t3", "t1")])], "t3"),
      "t3"
    );

    expect(result?.siblings.map((item) => item.turn_id)).toEqual(["t2", "t3"]);
    expect(result?.index).toBe(1);
  });

  it("没有分叉时返回 null，不显示版本切换", () => {
    const result = findSiblingBranches(
      tree([node("t1", null, [node("t2", "t1")])], "t2"),
      "t2"
    );

    expect(result).toBeNull();
  });
});
