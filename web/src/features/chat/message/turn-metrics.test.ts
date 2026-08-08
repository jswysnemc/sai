import { describe, expect, it } from "vitest";
import { formatTokenCount } from "./turn-metrics";

describe("formatTokenCount", () => {
  it("keeps small values and compacts larger token counts", () => {
    expect(formatTokenCount(999)).toBe("999");
    expect(formatTokenCount(1_250)).toBe("1.3k");
    expect(formatTokenCount(12_500)).toBe("13k");
  });
});
