import { describe, expect, it } from "vitest";
import { formatTurnElapsed } from "./live-run-indicator";

describe("formatTurnElapsed", () => {
  it("formats Chinese duration for turn progress", () => {
    expect(formatTurnElapsed(12_000, true)).toBe("12秒");
    expect(formatTurnElapsed(80_000, true)).toBe("1分20秒");
    expect(formatTurnElapsed(3_725_000, true)).toBe("1小时2分5秒");
  });

  it("formats English duration", () => {
    expect(formatTurnElapsed(12_000, false)).toBe("12s");
    expect(formatTurnElapsed(80_000, false)).toBe("1m 20s");
  });
});
