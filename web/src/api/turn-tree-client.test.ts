import { afterEach, describe, expect, it, vi } from "vitest";
import { loadTurnTree, treeFromIndex } from "./turn-tree-client";
import type { SessionTurnTreeIndex } from "./turn-tree-contracts";

/** 【会话分支】【接口样本】构造扁平线性节点；参数为数量，返回索引响应。 */
function index(count: number): SessionTurnTreeIndex {
  return {
    nodes: Array.from({ length: count }, (_, offset) => ({
      turn_id: `turn-${offset + 1}`, parent_turn_id: offset === 0 ? null : `turn-${offset}`,
      seq: offset + 1, user_summary: "question", assistant_summary: "answer", status: "completed", timestamp: "",
    })),
    active_leaf_id: `turn-${count}`, total_turns: count, branch_points: 0,
  };
}

describe("turn tree transport", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("rebuilds a 5000-turn hierarchy without recursive calls", () => {
    const wire = index(5000);
    const tree = treeFromIndex(wire);
    let count = 0;
    let node = tree.roots[0];
    while (node) {
      count++;
      node = node.children[0];
    }
    expect(count).toBe(5000);
    expect(tree.active_leaf_id).toBe("turn-5000");
    expect(wire.nodes[0]).not.toHaveProperty("children");
  });

  it("adapts the flat HTTP response to the existing view contract", async () => {
    const request = vi.fn().mockResolvedValue(new Response(JSON.stringify(index(2)), { status: 200 }));
    vi.stubGlobal("fetch", request);
    const tree = await loadTurnTree("session & one");
    expect(request.mock.calls[0][0]).toBe("/api/sessions/session%20%26%20one/turn-tree");
    expect(tree.roots[0].children[0].turn_id).toBe("turn-2");
  });

  it("accepts an older server response during an upgrade", async () => {
    const original = treeFromIndex(index(2));
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response(JSON.stringify(original), { status: 200 })));
    expect(await loadTurnTree("default")).toEqual(original);
  });
});
