import type { ReactNode } from "react";
import type { UsageStatsResponse } from "../../../api/contracts";
import type { Translate } from "./usage-labels";
import { UsageHeatmap } from "./usage-heatmap";
import { UsageInsightMetrics } from "./usage-insight-metrics";
import { UsageModelBreakdown } from "./usage-model-breakdown";
import { UsageRecentBars } from "./usage-recent-bars";

type Props = {
  data: UsageStatsResponse;
  ranking?: ReactNode;
  t: Translate;
  locale: "en-US" | "zh-CN";
};

/**
 * 【用量】【活动总览】指标、近 14 天、52 周热力图、模型新鲜/缓存明细与会话排行。
 * @param props 统计响应、排行插槽、双语函数与语言
 * @returns 活动总览
 */
export function UsageActivityOverview({ data, ranking, t, locale }: Props) {
  const hasUsage = data.summary.total_requests > 0 || data.summary.total_tokens > 0;
  if (!hasUsage) {
    return (
      <div className="usage-overview">
        <div className="usage-empty">{t("No usage in this range", "当前筛选范围内暂无用量")}</div>
        {ranking}
      </div>
    );
  }
  return (
    <div className="usage-overview">
      <UsageInsightMetrics data={data} t={t} locale={locale} />
      <UsageRecentBars days={data.daily_activity ?? []} locale={locale} t={t} />
      <UsageHeatmap days={data.daily_activity ?? []} locale={locale} t={t} />
      <UsageModelBreakdown rows={data.model_stats ?? []} t={t} />
      {ranking}
    </div>
  );
}
