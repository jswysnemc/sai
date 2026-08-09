import { describe, expect, it } from "vitest";
import { INITIAL_STACKED_PANE_STATE, reduceStackedPane } from "./stacked-pane-state";

describe("reduceStackedPane", () => {
  it("starts on the list pane", () => {
    expect(INITIAL_STACKED_PANE_STATE).toEqual({ pane: "list", direction: "forward" });
  });

  it("marks entering the detail pane as forward", () => {
    expect(reduceStackedPane({ pane: "list", direction: "forward" }, "detail")).toEqual({
      pane: "detail",
      direction: "forward"
    });
  });

  it("marks returning to the list pane as back", () => {
    expect(reduceStackedPane({ pane: "detail", direction: "forward" }, "list")).toEqual({
      pane: "list",
      direction: "back"
    });
  });

  it("keeps the identical state object when the target pane is unchanged", () => {
    const current = { pane: "detail", direction: "forward" } as const;
    expect(reduceStackedPane(current, "detail")).toBe(current);
  });
});
