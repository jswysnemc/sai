import { describe, expect, it } from "vitest";
import { contextCompactionThreshold } from "./context-budget-preview";

describe("configured compaction preview", () => {
  it("matches the backend contract for small and large windows", () => {
    expect(contextCompactionThreshold(20_000, 0.9, 50_000)).toBe(18_000);
    expect(contextCompactionThreshold(200_000, 0.9, 50_000)).toBe(180_000);
    expect(contextCompactionThreshold(1_000_000, 0.9, 50_000)).toBe(950_000);
    expect(contextCompactionThreshold(1_000_000, 0.9, 0)).toBe(900_000);
  });
  it("handles unknown budgets and clamps unsupported ratios", () => {
    expect(contextCompactionThreshold(0, 0.9, 50_000)).toBe(0);
    expect(contextCompactionThreshold(100, NaN, 0)).toBe(90);
    expect(contextCompactionThreshold(100, 0.1, 0)).toBe(50);
    expect(contextCompactionThreshold(100, 1, 0)).toBe(99);
  });
});
