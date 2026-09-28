import { DataTable, StatusBadge } from "../kit";
import type { UsageRecord } from "../../../api/contracts";
import { formatDuration, formatTime, formatTokens } from "./usage-format";
import { sourceLabel, statusLabel, type Translate } from "./usage-labels";

type UsageLogsTableProps = {
  logs: UsageRecord[];
  t: Translate;
  locale: "en-US" | "zh-CN";
};

/**
 * 渲染请求日志明细表。
 *
 * @param props 日志记录、双语取值函数与语言
 * @returns 日志表格，无数据时返回空态提示
 */
export function UsageLogsTable({ logs, t, locale }: UsageLogsTableProps) {
  return <DataTable label={t("Request logs", "请求日志")} rows={logs} rowKey={(record) => record.id} columns={[
    { id: "time", header: t("Time", "时间"), render: (record) => formatTime(record.created_at, locale) },
    { id: "source", header: t("Source", "来源"), render: (record) => <div>{sourceLabel(record.source, t)}<div className="text-xs text-muted">{record.operation}</div></div> },
    { id: "provider", header: t("Provider", "供应商"), render: (record) => record.provider_name || record.provider_id },
    { id: "model", header: t("Model", "模型"), render: (record) => record.model },
    { id: "input", header: t("In", "输入"), numeric: true, render: (record) => formatTokens(record.input_tokens) },
    { id: "cache", header: t("Cached", "缓存"), numeric: true, render: formatCacheDetail },
    { id: "output", header: t("Out", "输出"), numeric: true, render: (record) => formatTokens(record.output_tokens) },
    { id: "duration", header: t("Duration", "耗时"), numeric: true, render: (record) => formatDuration(record.duration_ms) },
    { id: "status", header: t("Status", "状态"), render: (record) => <div><StatusBadge tone={effectiveStatus(record) === "success" ? "success" : record.status === "error" ? "danger" : "neutral"}>{statusLabel(effectiveStatus(record), t)}</StatusBadge>{record.error_kind && <div className="text-xs">{record.error_kind}</div>}</div> }
  ]} />;
}

/**
 * 拼出单条记录的缓存读写明细。
 *
 * @param record 用量记录
 * @returns 读取量，写入量非零时追加显示；两者均缺失返回占位符
 */
function formatCacheDetail(record: UsageRecord) {
  const read = record.cache_read_tokens ?? 0;
  const write = record.cache_write_tokens ?? 0;
  if (read === 0 && write === 0) return "--";
  if (write === 0) return formatTokens(read);
  return `${formatTokens(read)} / ${formatTokens(write)}`;
}

/**
 * 计算日志行展示用的状态。
 *
 * @param record 用量记录
 * @returns 成功但无用量上报时归入 missing_usage，否则沿用原状态
 */
function effectiveStatus(record: UsageRecord) {
  return record.status === "success" && record.usage_source === "missing" ? "missing_usage" : record.status;
}
