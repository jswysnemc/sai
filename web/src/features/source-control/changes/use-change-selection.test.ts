import { describe, expect, it } from "vitest";
import { updateChangeSelection } from "./use-change-selection";

describe("updateChangeSelection", () => {
  const paths = ["a.ts", "b.ts", "c.ts", "d.ts"];

  it("replaces or toggles individual paths", () => {
    expect([...updateChangeSelection(new Set(["a.ts"]), paths, "a.ts", "c.ts", { toggle: false, range: false })]).toEqual(["c.ts"]);
    expect([...updateChangeSelection(new Set(["a.ts"]), paths, "a.ts", "c.ts", { toggle: true, range: false })]).toEqual(["a.ts", "c.ts"]);
    expect([...updateChangeSelection(new Set(["a.ts"]), paths, "a.ts", "a.ts", { toggle: true, range: false })]).toEqual([]);
  });

  it("selects an ordered range in either direction", () => {
    expect([...updateChangeSelection(new Set(), paths, "a.ts", "c.ts", { toggle: false, range: true })]).toEqual(["a.ts", "b.ts", "c.ts"]);
    expect([...updateChangeSelection(new Set(), paths, "d.ts", "b.ts", { toggle: false, range: true })]).toEqual(["b.ts", "c.ts", "d.ts"]);
  });
});
