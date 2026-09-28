import type { UsageGroupStats } from "../../../api/contracts";
import { DataTable, ShareBar, type DataColumn } from "../kit";
import { formatCount, formatDuration, formatPercent, formatTime, formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";

type Props = { rows: UsageGroupStats[]; type: "provider" | "model"; t: Translate; locale: "en-US" | "zh-CN"; compact?: boolean };

/**
 * 【用量】【分组对比】以相同口径展示供应商或模型统计，保留计费与上报差异。
 * @param props 统计行、维度、语言与紧凑模式
 * @returns 支持列排序的统计表
 */
export function UsageGroupTable({ rows, type, t, locale, compact }: Props) {
  const total = rows.reduce((sum, row) => sum + row.total_tokens, 0);
  const columns: DataColumn<UsageGroupStats>[] = [
    { id: "label", header: type === "provider" ? t("Provider", "供应商") : t("Model", "模型"), sortValue: (row) => row.label, render: (row) => <div><strong>{row.label}</strong>{type === "model" && row.provider_name && <div className="text-xs text-muted">{row.provider_name}</div>}</div> },
    { id: "requests", header: t("Req", "请求"), numeric: true, sortValue: (row) => row.request_count, render: (row) => formatCount(row.request_count) },
    { id: "success", header: t("Success", "成功率"), numeric: true, render: (row) => formatPercent(row.success_count, row.request_count) },
    { id: "billable", header: t("Billable in", "计费输入"), numeric: true, sortValue: (row) => row.billable_input_tokens, render: (row) => <ShareBar ratio={total ? row.total_tokens / total : 0} label={formatTokens(row.billable_input_tokens)} /> },
    { id: "input", header: t("Reported in", "上报输入"), numeric: true, sortValue: (row) => row.input_tokens, render: (row) => formatTokens(row.input_tokens) },
    { id: "output", header: t("Out", "输出"), numeric: true, sortValue: (row) => row.output_tokens, render: (row) => formatTokens(row.output_tokens) }
  ];
  if (!compact) columns.push(
    { id: "cache", header: t("Cache", "缓存"), numeric: true, render: (row) => formatPercent(row.cache_read_tokens, row.input_tokens) },
    { id: "duration", header: t("Avg", "均耗时"), numeric: true, render: (row) => formatDuration(row.average_duration_ms) },
    { id: "last", header: t("Last", "最近"), render: (row) => formatTime(row.last_used_at, locale) }
  );
  return <DataTable label={type === "provider" ? t("Provider comparison", "供应商对比") : t("Model comparison", "模型对比")} rows={rows} rowKey={(row) => row.id} columns={columns} />;
}
