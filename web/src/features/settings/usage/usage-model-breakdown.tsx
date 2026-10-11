import type { UsageGroupStats } from "../../../api/contracts";
import { DataTable, SettingsPanel, type DataColumn } from "../kit";
import { formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";
import { freshTokens } from "./usage-activity";

type Props = {
  rows: UsageGroupStats[];
  t: Translate;
};

/**
 * 【用量】【模型明细】按模型列出新鲜量、缓存读取与计费输入，不展示美元。
 * @param props 模型统计与双语函数
 * @returns 明细表；无数据时不渲染
 */
export function UsageModelBreakdown({ rows, t }: Props) {
  const visible = rows.filter((row) => row.total_tokens > 0);
  if (visible.length === 0) return null;
  const columns: DataColumn<UsageGroupStats>[] = [
    {
      id: "model",
      header: t("Model", "模型"),
      sortValue: (row) => row.label,
      render: (row) => (
        <div>
          <strong title={row.model ?? row.label}>{row.label}</strong>
          {row.provider_name && <div className="text-xs text-muted">{row.provider_name}</div>}
        </div>
      )
    },
    {
      id: "fresh",
      header: t("Fresh", "新鲜"),
      numeric: true,
      sortValue: (row) => freshTokens(row.input_tokens, row.output_tokens, row.cache_read_tokens),
      render: (row) => formatTokens(freshTokens(row.input_tokens, row.output_tokens, row.cache_read_tokens))
    },
    {
      id: "cached",
      header: t("Cached", "缓存"),
      numeric: true,
      sortValue: (row) => row.cache_read_tokens,
      render: (row) => formatTokens(row.cache_read_tokens)
    },
    {
      id: "billable",
      header: t("Billable in", "计费输入"),
      numeric: true,
      sortValue: (row) => row.billable_input_tokens,
      render: (row) => formatTokens(row.billable_input_tokens)
    }
  ];
  return (
    <SettingsPanel title={t("Model usage", "模型用量")}>
      <DataTable label={t("Model usage", "模型用量")} rows={visible} rowKey={(row) => row.id} columns={columns} />
    </SettingsPanel>
  );
}
