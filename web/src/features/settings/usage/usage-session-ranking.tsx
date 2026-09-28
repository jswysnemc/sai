import type { UsageSessionSort, UsageSessionStats } from "../../../api/contracts/usage";
import { Button } from "../../../shared/ui/button/button";
import { DataTable, SettingsPanel, SkSelect } from "../kit";
import { formatCount, formatTime, formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";

type Props = {
  rows: UsageSessionStats[]; total: number; sort: UsageSessionSort; limit: number;
  onSortChange: (sort: UsageSessionSort) => void; onLimitChange: (limit: number) => void;
  onSelect?: (session: UsageSessionStats) => void;
  workspaces?: Array<{ id: string; name: string; path: string }>;
  t: Translate; locale: "en-US" | "zh-CN"; compact?: boolean;
};

/**
 * 【用量】【会话排行】展示会话用量与可读工作区，支持查看对应日志。
 * @param props 排行、排序、工作区名称与交互回调
 * @returns 会话排行面板
 */
export function UsageSessionRanking({ rows, total, sort, limit, onSortChange, onLimitChange, onSelect, workspaces = [], t, locale, compact }: Props) {
  return <SettingsPanel title={t("Top-consuming sessions", "高消耗会话")} description={t(`${formatCount(total)} sessions in the selected range`, `当前筛选范围共 ${formatCount(total)} 个会话`)} actions={<div className="flex flex-wrap gap-2">
    <SkSelect value={sort} ariaLabel={t("Rank sessions by", "会话排名指标")} options={[{ value: "total_tokens", label: t("Total tokens", "总 Token") }, { value: "billable_tokens", label: t("Billable tokens", "计费 Token") }, { value: "requests", label: t("Requests", "请求次数") }]} onChange={onSortChange} />
    <SkSelect value={String(limit)} ariaLabel={t("Ranking size", "排行数量")} options={[10, 20, 50].map((value) => ({ value: String(value), label: t(`Top ${value}`, `前 ${value} 名`) }))} onChange={(value) => onLimitChange(Number(value))} />
  </div>}>
    {!rows.length ? <p className="text-xs text-muted">{t("No session usage in this range", "当前筛选范围内暂无会话用量")}</p> : <DataTable label={t("Session ranking", "会话排行")} rows={rows} rowKey={(row) => `${row.workspace_id ?? ""}:${row.session_id}`} columns={[
      { id: "session", header: t("Session", "会话"), render: (row) => {
        const workspace = workspaces.find((item) => item.id === row.workspace_id);
        return <div className="sk-cell-main">
          {onSelect ? <Button variant="ghost" className="whitespace-normal text-left" onClick={() => onSelect(row)}>{row.title || row.session_id}</Button> : <strong>{row.title || row.session_id}</strong>}
          <small>{row.session_id}</small>
          {row.workspace_id && <small title={workspace?.path ?? row.workspace_id}>{workspace?.name ?? t("Unregistered workspace", "未登记工作区")}</small>}
        </div>;
      } },
      { id: "tokens", header: t("Total tokens", "总 Token"), numeric: true, render: (row) => formatTokens(row.total_tokens) },
      { id: "billable", header: t("Billable tokens", "计费 Token"), numeric: true, render: (row) => formatTokens(row.billable_total_tokens) },
      { id: "requests", header: t("Requests", "请求"), numeric: true, render: (row) => <div>{formatCount(row.total_requests)}{row.missing_usage_requests > 0 && <small className="block whitespace-normal text-xs">{t(`${formatCount(row.missing_usage_requests)} without token usage`, `${formatCount(row.missing_usage_requests)} 次未上报用量`)}</small>}</div> },
      ...(!compact ? [{ id: "io", header: t("Input / output", "输入 / 输出"), numeric: true, render: (row: UsageSessionStats) => `${formatTokens(row.input_tokens)} / ${formatTokens(row.output_tokens)}` }, { id: "last", header: t("Last used", "最近调用"), render: (row: UsageSessionStats) => formatTime(row.last_used_at, locale) }] : [])
    ]} />}
  </SettingsPanel>;
}
