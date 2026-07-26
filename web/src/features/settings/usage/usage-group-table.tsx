import type { UsageGroupStats } from "../../../api/contracts";
import { formatCount, formatDuration, formatPercent, formatTime, formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";

type UsageGroupTableProps = {
  rows: UsageGroupStats[];
  type: "provider" | "model";
  t: Translate;
  locale: "en-US" | "zh-CN";
  compact?: boolean;
};

/**
 * 渲染供应商或模型维度的统计表格。
 *
 * 输入列同时给出上报量与等效计费量，缓存折扣带来的差距可逐行核对。
 *
 * @param props 统计行、维度类型、双语取值函数、语言与紧凑模式
 * @returns 统计表格，无数据时返回空态提示
 */
export function UsageGroupTable({ rows, type, t, locale, compact }: UsageGroupTableProps) {
  if (rows.length === 0) {
    return <div className="usage-empty">{t("No usage data", "暂无统计数据")}</div>;
  }
  return (
    <div className="usage-table-wrap">
      <table className="usage-table">
        <thead>
          <tr>
            <th>{type === "provider" ? t("Provider", "供应商") : t("Model", "模型")}</th>
            <th>{t("Req", "请求")}</th>
            <th>{t("Success", "成功率")}</th>
            <th>{t("Billable in", "计费输入")}</th>
            <th>{t("Reported in", "上报输入")}</th>
            <th>{t("Out", "输出")}</th>
            {!compact && <th>{t("Cache", "缓存")}</th>}
            {!compact && <th>{t("Avg", "均耗时")}</th>}
            {!compact && <th>{t("Last", "最近")}</th>}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.id}>
              <td>
                <strong>{row.label}</strong>
                {type === "model" && row.provider_name && <small>{row.provider_name}</small>}
              </td>
              <td>{formatCount(row.request_count)}</td>
              <td>{formatPercent(row.success_count, row.request_count)}</td>
              <td className="usage-cell-accent">{formatTokens(row.billable_input_tokens)}</td>
              <td>{formatTokens(row.input_tokens)}</td>
              <td>{formatTokens(row.output_tokens)}</td>
              {!compact && <td>{formatPercent(row.cache_read_tokens, row.input_tokens)}</td>}
              {!compact && <td>{formatDuration(row.average_duration_ms)}</td>}
              {!compact && <td>{formatTime(row.last_used_at, locale)}</td>}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
