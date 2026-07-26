import { Info } from "lucide-react";
import type { UsageSummary } from "../../../api/contracts";
import { formatCount, formatDuration, formatPercent, formatRatio, formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";

type UsageSummaryTilesProps = {
  summary: UsageSummary;
  t: Translate;
};

/**
 * 渲染用量汇总卡片组。
 *
 * 主指标采用等效计费口径，与供应商账单可比；原始上报量作为副标同时给出，
 * 缓存命中率高时两者会相差数倍，卡片下方的说明条解释该差异。
 *
 * @param props 汇总数据与双语取值函数
 * @returns 汇总卡片组
 */
export function UsageSummaryTiles({ summary, t }: UsageSummaryTilesProps) {
  const cacheRatio = formatPercent(summary.cache_read_tokens, summary.input_tokens);
  const ratio = formatRatio(summary.input_tokens, summary.billable_input_tokens);
  return (
    <>
      <div className="usage-summary-grid">
        <SummaryTile
          label={t("Requests", "请求数")}
          value={formatCount(summary.total_requests)}
          sub={`${formatCount(summary.successful_requests)} ${t("ok", "成功")} · ${formatCount(summary.failed_requests)} ${t("failed", "失败")}`}
        />
        <SummaryTile
          label={t("Billable tokens", "计费 Token")}
          value={formatTokens(summary.billable_total_tokens)}
          sub={`${formatTokens(summary.billable_input_tokens)} / ${formatTokens(summary.output_tokens)}`}
          accent
        />
        <SummaryTile
          label={t("Reported tokens", "上报 Token")}
          value={formatTokens(summary.total_tokens)}
          sub={ratio ? `${formatTokens(summary.input_tokens)} ${t("in", "输入")} · ${ratio}` : `${formatTokens(summary.input_tokens)} ${t("in", "输入")}`}
        />
        <SummaryTile
          label={t("Cache hit", "缓存命中")}
          value={cacheRatio}
          sub={`${formatTokens(summary.cache_read_tokens)} ${t("read", "读取")} · ${formatTokens(summary.cache_write_tokens)} ${t("write", "写入")}`}
        />
        <SummaryTile
          label={t("Avg duration", "平均耗时")}
          value={formatDuration(summary.average_duration_ms)}
          sub={`${formatCount(summary.missing_usage_requests)} ${t("without usage", "无用量")}`}
        />
      </div>
      {ratio && (
        <p className="usage-billing-note">
          <Info size={13} />
          {t(
            `Reported input is ${ratio} the billable amount: cached reads are billed at a fraction of the standard input price.`,
            `上报输入量是计费量的 ${ratio}：命中缓存的读取按标准输入价的一小部分计费。`
          )}
        </p>
      )}
    </>
  );
}

/**
 * 渲染单张汇总卡片。
 *
 * @param props 标签、主值、副标与是否强调
 * @returns 汇总卡片
 */
function SummaryTile({
  label,
  value,
  sub,
  accent,
}: {
  label: string;
  value: string;
  sub?: string;
  accent?: boolean;
}) {
  return (
    <div className={accent ? "usage-summary-tile accent" : "usage-summary-tile"}>
      <small>{label}</small>
      <strong>{value}</strong>
      {sub && <i>{sub}</i>}
    </div>
  );
}
