import type { UsageGroupStats, UsageStatsResponse } from "../../../api/contracts";
import { SettingsPanel } from "../kit";
import { formatCount, formatPercent, formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";
import { formatDateLabel, freshTokens } from "./usage-activity";

type Props = {
  data: UsageStatsResponse;
  t: Translate;
  locale: "en-US" | "zh-CN";
};

type Metric = {
  label: string;
  value: string;
  detail?: string;
};

/**
 * 【用量】【指标】顶部卡片与洞察列表，只展示已有 Token / 请求口径，不编造费用。
 * @param props 统计响应、双语函数与语言
 * @returns 指标卡片与洞察列表
 */
export function UsageInsightMetrics({ data, t, locale }: Props) {
  const summary = data.summary;
  const days = data.daily_activity ?? [];
  const streaks = data.streaks ?? { current_streak: 0, longest_streak: 0 };
  const peakTokens = days.reduce((peak, day) => Math.max(peak, day.tokens), 0);
  const fresh = freshTokens(summary.input_tokens, summary.output_tokens, summary.cache_read_tokens);
  const cached = summary.cache_read_tokens;
  const topModel = topModelRow(data.model_stats ?? []);
  const cards: Metric[] = [
    {
      label: t("Total tokens", "总 Token"),
      value: formatTokens(summary.total_tokens),
      detail: t(`${formatCount(data.active_days ?? 0)} active days`, `${formatCount(data.active_days ?? 0)} 个活跃日`)
    },
    {
      label: t("Peak tokens", "峰值 Token"),
      value: formatTokens(peakTokens),
      detail: data.peak_activity_day ? formatDateLabel(data.peak_activity_day, locale) : undefined
    },
    {
      label: t("Current streak", "当前连续"),
      value: formatDayCount(streaks.current_streak, t),
      detail: streaks.current_streak_start
        ? t(`since ${formatDateLabel(streaks.current_streak_start, locale)}`, `自 ${formatDateLabel(streaks.current_streak_start, locale)}`)
        : undefined
    },
    {
      label: t("Longest streak", "最长连续"),
      value: formatDayCount(streaks.longest_streak, t),
      detail: streaks.longest_streak_start && streaks.longest_streak_end
        ? `${formatDateLabel(streaks.longest_streak_start, locale)} – ${formatDateLabel(streaks.longest_streak_end, locale)}`
        : undefined
    },
    {
      label: t("Requests", "请求数"),
      value: formatCount(summary.total_requests),
      detail: `${formatCount(summary.successful_requests)} ${t("ok", "成功")} · ${formatCount(summary.failed_requests)} ${t("failed", "失败")}`
    }
  ];
  const insights: Metric[] = [
    {
      label: t("Active rate", "活跃比例"),
      value: formatPercent(data.active_days ?? 0, data.total_days ?? 0)
    },
    {
      label: t("Most used model", "常用模型"),
      value: topModel?.label ?? t("None", "无"),
      detail: topModel ? `${formatTokens(topModel.total_tokens)} ${t("tokens", "Token")}` : undefined
    },
    {
      label: t("Fresh tokens", "新鲜 Token"),
      value: formatTokens(fresh),
      detail: t(`${formatPercent(fresh, summary.total_tokens)} of total`, `占总 ${formatPercent(fresh, summary.total_tokens)}`)
    },
    {
      label: t("Cached tokens", "缓存 Token"),
      value: formatTokens(cached),
      detail: t(`${formatPercent(cached, summary.input_tokens)} of input`, `占输入 ${formatPercent(cached, summary.input_tokens)}`)
    },
    {
      label: t("Billable tokens", "计费 Token"),
      value: formatTokens(summary.billable_total_tokens),
      detail: t("Cache-discounted equivalent, not a dollar amount.", "按缓存折扣折算的等效量，不是美元费用。")
    },
    {
      label: t("Total sessions", "会话数"),
      value: formatCount(data.total_sessions)
    }
  ];

  return (
    <>
      <div className="usage-summary-grid">
        {cards.map((metric, index) => (
          <div className={index === 0 ? "usage-summary-tile accent" : "usage-summary-tile"} key={metric.label}>
            <small>{metric.label}</small>
            <strong>{metric.value}</strong>
            {metric.detail && <i>{metric.detail}</i>}
          </div>
        ))}
      </div>
      <SettingsPanel title={t("Activity insights", "活动洞察")}>
        <dl className="usage-insight-list">
          {insights.map((metric) => (
            <div key={metric.label}>
              <dt>{metric.label}</dt>
              <dd>
                {metric.detail && <span>{metric.detail}</span>}
                <strong>{metric.value}</strong>
              </dd>
            </div>
          ))}
        </dl>
      </SettingsPanel>
    </>
  );
}

function topModelRow(rows: UsageGroupStats[]): UsageGroupStats | undefined {
  return rows.reduce<UsageGroupStats | undefined>((best, row) => {
    if (row.total_tokens <= 0) return best;
    if (!best || row.total_tokens > best.total_tokens) return row;
    return best;
  }, undefined);
}

function formatDayCount(value: number, t: Translate): string {
  return t(value === 1 ? "1 day" : `${formatCount(value)} days`, `${formatCount(value)} 天`);
}
