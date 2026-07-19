import { describe, expect, it } from "vitest";
import { calculateGitGraphWindow } from "./graph-window";

describe("calculateGitGraphWindow", () => {
  it("renders the first viewport with overscan", () => {
    expect(calculateGitGraphWindow(1000, 56, 0, 560, 4)).toEqual({
      start: 0,
      end: 14,
      offsetTop: 0,
      totalHeight: 56000
    });
  });

  it("keeps only the rows near a deep scroll position", () => {
    expect(calculateGitGraphWindow(1000, 56, 28000, 560, 4)).toEqual({
      start: 496,
      end: 514,
      offsetTop: 27776,
      totalHeight: 56000
    });
  });

  it("clamps an outdated scroll position to the final rows", () => {
    expect(calculateGitGraphWindow(12, 56, 5000, 280, 3)).toEqual({
      start: 8,
      end: 12,
      offsetTop: 448,
      totalHeight: 672
    });
  });

  it("returns an empty window for an empty list", () => {
    expect(calculateGitGraphWindow(0, 56, 0, 0)).toEqual({
      start: 0,
      end: 0,
      offsetTop: 0,
      totalHeight: 0
    });
  });
});
