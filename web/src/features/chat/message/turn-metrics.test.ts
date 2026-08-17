import { describe, expect, it } from "vitest";
import { formatTokenCount, formatTokensPerSec, formatTtft } from "./turn-metrics";

describe("formatTokenCount", () => {
  it("keeps small values and compacts larger token counts", () => {
    expect(formatTokenCount(999)).toBe("999");
    expect(formatTokenCount(1_250)).toBe("1.3k");
    expect(formatTokenCount(12_500)).toBe("13k");
  });
});

describe("formatTtft", () => {
  it("uses milliseconds under one second and compact seconds after", () => {
    expect(formatTtft(420, true)).toBe("420ms");
    expect(formatTtft(1_200, true)).toBe("1.2秒");
    expect(formatTtft(1_200, false)).toBe("1.2s");
  });
});

describe("formatTokensPerSec", () => {
  it("derives output rate from completion tokens and generation time", () => {
    expect(formatTokensPerSec(4_000, 12_500)).toBe("320");
    expect(formatTokensPerSec(20, 5_000)).toBe("4.0");
    expect(formatTokensPerSec(0, 5_000)).toBeNull();
  });
});
