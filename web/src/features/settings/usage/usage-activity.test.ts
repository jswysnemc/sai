import { describe, expect, it } from "vitest";
import type { UsageDailyActivity } from "../../../api/contracts/usage";
import {
  buildHeatmapDays,
  buildMonthLabels,
  buildRecentDays,
  freshTokens,
  getHeatLevel
} from "./usage-activity";

function day(date: string, patch: Partial<UsageDailyActivity> = {}): UsageDailyActivity {
  return {
    date,
    requests: 0,
    sessions: 0,
    tokens: 0,
    input_tokens: 0,
    output_tokens: 0,
    cache_read_tokens: 0,
    billable_input_tokens: 0,
    ...patch
  };
}

describe("usage activity helpers", () => {
  it("keeps empty days at heat level 0 and never invents future cells", () => {
    const days = [
      day("2026-05-07"),
      day("2026-05-08", { requests: 2, sessions: 1, tokens: 64_000 }),
      day("2026-05-09", { requests: 4, sessions: 2, tokens: 128_000 })
    ];
    const cells = buildHeatmapDays(days, "daily");
    expect(cells).toHaveLength(3);
    expect(cells[0].level).toBe(0);
    expect(cells[0].date).toBe("2026-05-07");
    expect(cells[2].date).toBe("2026-05-09");
    expect(cells.every((cell) => cell.date <= "2026-05-09")).toBe(true);
    expect(getHeatLevel(days[0], 10)).toBe(0);
  });

  it("aggregates weekly and cumulative bars from padded empty days", () => {
    const days = Array.from({ length: 14 }, (_, index) => {
      const date = `2026-04-${String(index + 1).padStart(2, "0")}`;
      return index === 10 ? day(date, { requests: 3, sessions: 1, tokens: 50_000 }) : day(date);
    });
    const weekly = buildHeatmapDays(days, "weekly");
    expect(weekly).toHaveLength(14);
    const secondWeek = weekly.slice(7);
    expect(secondWeek.some((cell) => cell.level > 0)).toBe(true);
    expect(secondWeek.every((cell) => cell.rangeStart === "2026-04-08")).toBe(true);
    const cumulative = buildHeatmapDays(days, "cumulative");
    expect(cumulative[13].tokens).toBe(50_000);
  });

  it("places month labels without colliding or overhanging the last two weeks", () => {
    const days = Array.from({ length: 52 * 7 }, (_, index) => {
      const date = new Date("2025-05-11T00:00:00");
      date.setDate(date.getDate() + index);
      const year = date.getFullYear();
      const month = `${date.getMonth() + 1}`.padStart(2, "0");
      const dayOfMonth = `${date.getDate()}`.padStart(2, "0");
      return day(`${year}-${month}-${dayOfMonth}`);
    });
    const labels = buildMonthLabels(buildHeatmapDays(days, "daily"), "en-US");
    expect(labels.length).toBeGreaterThan(0);
    expect(labels.every((label, index) => index === 0 || label.week - labels[index - 1].week >= 3)).toBe(true);
    expect(labels[labels.length - 1].week).toBeLessThan(51);
  });

  it("takes the trailing window for recent bars and computes fresh tokens without inventing cost", () => {
    const days = [day("2026-05-01", { tokens: 1 }), day("2026-05-02"), day("2026-05-03", { tokens: 9 })];
    expect(buildRecentDays(days, 2).map((item) => item.date)).toEqual(["2026-05-02", "2026-05-03"]);
    expect(freshTokens(100_000, 1_000, 95_000)).toBe(6_000);
    expect(freshTokens(10, 2, 40)).toBe(2);
  });
});
