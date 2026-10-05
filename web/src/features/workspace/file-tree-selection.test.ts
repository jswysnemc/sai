import { describe, expect, it } from "vitest";
import { contextTreeSelection, EMPTY_TREE_SELECTION, nextTreeSelection, outermostPaths, pruneTreeSelection } from "./file-tree-selection";

const order = ["a", "a/x", "a/y", "b", "c"];
const plain = { toggle: false, range: false };

describe("file tree selection", () => {
  it("plain click replaces the selection", () => {
    const first = nextTreeSelection(EMPTY_TREE_SELECTION, "a", order, plain);
    const second = nextTreeSelection(first, "b", order, plain);
    expect([...second.paths]).toEqual(["b"]);
    expect(second.anchor).toBe("b");
  });

  it("ctrl click toggles items freely", () => {
    let state = nextTreeSelection(EMPTY_TREE_SELECTION, "a", order, plain);
    state = nextTreeSelection(state, "c", order, { toggle: true, range: false });
    state = nextTreeSelection(state, "a/y", order, { toggle: true, range: false });
    expect([...state.paths].sort()).toEqual(["a", "a/y", "c"]);
    state = nextTreeSelection(state, "a", order, { toggle: true, range: false });
    expect([...state.paths].sort()).toEqual(["a/y", "c"]);
  });

  it("shift click selects the visible range from the anchor", () => {
    let state = nextTreeSelection(EMPTY_TREE_SELECTION, "a/x", order, plain);
    state = nextTreeSelection(state, "b", order, { toggle: false, range: true });
    expect([...state.paths]).toEqual(["a/x", "a/y", "b"]);
    // 反向区间与锚点保持不变
    state = nextTreeSelection(state, "a", order, { toggle: false, range: true });
    expect([...state.paths]).toEqual(["a", "a/x"]);
    expect(state.anchor).toBe("a/x");
  });

  it("right click keeps an existing multi-selection", () => {
    const state = { paths: new Set(["a", "c"]), anchor: "c" };
    expect(contextTreeSelection(state, "a")).toBe(state);
    expect([...contextTreeSelection(state, "b").paths]).toEqual(["b"]);
  });

  it("collapses children under a selected folder and prunes missing paths", () => {
    expect(outermostPaths(["a/x", "a", "c", "a/y"])).toEqual(["a", "c"]);
    const state = { paths: new Set(["a", "c"]), anchor: "c" };
    expect([...pruneTreeSelection(state, (path) => path !== "c").paths]).toEqual(["a"]);
  });
});
