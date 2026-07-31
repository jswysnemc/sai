import { describe, expect, it } from "vitest";
import {
  SOURCE_CONTROL_SPLIT_DEFAULT_WIDTH,
  SOURCE_CONTROL_SPLIT_MIN_DETAIL_WIDTH,
  SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH,
  clampSourceControlListWidth,
  parseSourceControlListWidth,
  shouldStackSourceControlSplit
} from "./source-control-split-state";

describe("source control split state", () => {
  it("preserves minimum space for both Git panes", () => {
    expect(clampSourceControlListWidth(80, 900)).toBe(SOURCE_CONTROL_SPLIT_MIN_LIST_WIDTH);
    expect(clampSourceControlListWidth(700, 900)).toBe(900 - SOURCE_CONTROL_SPLIT_MIN_DETAIL_WIDTH);
    expect(clampSourceControlListWidth(360, 900)).toBe(360);
  });

  it("parses persisted widths and rejects invalid values", () => {
    expect(parseSourceControlListWidth("384")).toBe(384);
    expect(parseSourceControlListWidth("invalid")).toBe(SOURCE_CONTROL_SPLIT_DEFAULT_WIDTH);
    expect(parseSourceControlListWidth(null)).toBe(SOURCE_CONTROL_SPLIT_DEFAULT_WIDTH);
  });

  it("stacks panes when the container cannot preserve both minimum widths", () => {
    expect(shouldStackSourceControlSplit(539)).toBe(true);
    expect(shouldStackSourceControlSplit(540)).toBe(false);
    expect(shouldStackSourceControlSplit(900)).toBe(false);
  });
});
