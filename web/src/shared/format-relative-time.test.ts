import { describe, expect, it } from "vitest";
import { formatRelativeTime } from "./format-relative-time";

describe("formatRelativeTime", () => {
  const now = Date.parse("2026-07-18T12:00:00.000Z");

  it("formats just now", () => {
    expect(formatRelativeTime(now - 10_000, "zh-CN", now)).toBe("刚刚");
  });

  it("formats minutes", () => {
    const text = formatRelativeTime(now - 5 * 60_000, "zh-CN", now);
    expect(text).toMatch(/5/);
  });
});
