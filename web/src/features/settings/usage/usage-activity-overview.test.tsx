import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { UsageDailyActivity, UsageGroupStats, UsageStatsResponse, UsageSummary } from "../../../api/contracts/usage";
import { UsageActivityOverview } from "./usage-activity-overview";

function summary(patch: Partial<UsageSummary> = {}): UsageSummary {
  return {
    total_requests: 12,
    successful_requests: 11,
    failed_requests: 1,
    missing_usage_requests: 0,
    provider_reported_requests: 12,
    total_tokens: 200_000,
    input_tokens: 180_000,
    output_tokens: 20_000,
    cache_read_tokens: 90_000,
    cache_write_tokens: 0,
    billable_input_tokens: 99_000,
    billable_total_tokens: 119_000,
    ...patch
  };
}

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

function model(id: string, label: string, input: number, cached: number): UsageGroupStats {
  return {
    id,
    label,
    provider_id: "p1",
    provider_name: "Provider",
    model: id,
    request_count: 4,
    success_count: 4,
    total_tokens: input + 10,
    input_tokens: input,
    output_tokens: 10,
    cache_read_tokens: cached,
    billable_input_tokens: input - cached + Math.round(cached * 0.1)
  };
}

function stats(patch: Partial<UsageStatsResponse> = {}): UsageStatsResponse {
  return {
    summary: summary(),
    trend: [],
    logs: [],
    provider_stats: [],
    model_stats: [model("claude", "claude", 180_000, 90_000)],
    session_stats: [],
    total_sessions: 3,
    total_logs: 12,
    skipped_records: 0,
    daily_activity: [
      day("2026-05-07"),
      day("2026-05-08", { requests: 2, sessions: 1, tokens: 64_000, input_tokens: 50_000 }),
      day("2026-05-09", { requests: 4, sessions: 2, tokens: 128_000, input_tokens: 100_000, cache_read_tokens: 40_000 })
    ],
    streaks: { current_streak: 2, longest_streak: 2, current_streak_start: "2026-05-08", longest_streak_start: "2026-05-08", longest_streak_end: "2026-05-09" },
    active_days: 2,
    total_days: 3,
    peak_activity_day: "2026-05-09",
    ...patch
  };
}

describe("usage activity overview", () => {
  it("renders heatmap empty-day cells, insights and fresh/cache model rows without USD", () => {
    const html = renderToStaticMarkup(
      <UsageActivityOverview data={stats()} t={(_en, zh) => zh} locale="zh-CN" />
    );
    expect(html).toContain("总 Token");
    expect(html).toContain("当前连续");
    expect(html).toContain("活动洞察");
    expect(html).toContain("新鲜 Token");
    expect(html).toContain("缓存 Token");
    expect(html).toContain("计费 Token");
    expect(html).toContain("按缓存折扣折算的等效量，不是美元费用。");
    expect(html).not.toContain("$");
    expect(html).not.toContain("USD");
    expect(html).toContain("Token 活动");
    expect(html).toContain("role=\"grid\"");
    expect(html).toContain("data-date=\"2026-05-07\"");
    expect(html).toContain("claude");
    expect(html).toContain("新鲜");
    expect(html).toContain("缓存");
  });

  it("explains an empty filtered range without drawing a heatmap", () => {
    const html = renderToStaticMarkup(
      <UsageActivityOverview
        data={stats({
          summary: summary({ total_requests: 0, total_tokens: 0, input_tokens: 0, output_tokens: 0, billable_total_tokens: 0, billable_input_tokens: 0, cache_read_tokens: 0 }),
          daily_activity: [day("2026-05-09")],
          model_stats: [],
          active_days: 0,
          streaks: { current_streak: 0, longest_streak: 0 }
        })}
        t={(_en, zh) => zh}
        locale="zh-CN"
      />
    );
    expect(html).toContain("当前筛选范围内暂无用量");
    expect(html).not.toContain("role=\"grid\"");
  });
});
