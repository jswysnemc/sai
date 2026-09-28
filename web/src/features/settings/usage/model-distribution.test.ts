import { describe, expect, it } from "vitest";
import type { UsageGroupStats } from "../../../api/contracts";
import { buildModelDistribution } from "./model-distribution";

describe("model distribution totals", () => {
  it("keeps long-tail usage in the total and the other slice", () => {
    const rows = Array.from({ length: 8 }, (_, index) => ({ id: String(index), label: String(index), total_tokens: 10 })) as UsageGroupStats[];
    const result = buildModelDistribution(rows, "其他");
    expect(result.total).toBe(80);
    expect(result.slices).toHaveLength(6);
    expect(result.slices.at(-1)?.total_tokens).toBe(30);
    expect(result.slices.reduce((sum, row) => sum + row.total_tokens, 0)).toBe(result.total);
    expect(rows).toHaveLength(8);
  });
});
