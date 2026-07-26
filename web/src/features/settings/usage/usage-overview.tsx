import type { UsageStatsResponse } from "../../../api/contracts";
import { UsageGroupTable } from "./usage-group-table";
import type { Translate } from "./usage-labels";
import { UsageModelDonut } from "./usage-model-donut";
import { UsageSummaryTiles } from "./usage-summary-tiles";
import { UsageTrendChart } from "./usage-trend-chart";

type UsageOverviewProps = {
  data: UsageStatsResponse;
  t: Translate;
  locale: "en-US" | "zh-CN";
};

const TOP_MODEL_LIMIT = 8;

/**
 * 渲染用量总览：汇总卡片、趋势、模型分布与排行。
 *
 * @param props 统计响应、双语取值函数与语言
 * @returns 总览面板
 */
export function UsageOverview({ data, t, locale }: UsageOverviewProps) {
  return (
    <div className="usage-overview">
      <UsageSummaryTiles summary={data.summary} t={t} />
      <div className="usage-panel-row">
        <section className="usage-panel">
          <h3>{t("Token trend", "Token 趋势")}</h3>
          <UsageTrendChart points={data.trend} t={t} />
        </section>
        <section className="usage-panel">
          <h3>{t("Model distribution", "模型分布")}</h3>
          <UsageModelDonut rows={data.model_stats} t={t} />
        </section>
      </div>
      <section className="usage-panel">
        <h3>{t("Top models", "模型排行")}</h3>
        <UsageGroupTable rows={data.model_stats.slice(0, TOP_MODEL_LIMIT)} type="model" t={t} locale={locale} compact />
      </section>
    </div>
  );
}
