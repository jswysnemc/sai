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
  if (logs.length === 0) {
    return <div className="usage-empty">{t("No request logs", "暂无请求日志")}</div>;
  }
  return (
    <div className="usage-table-wrap">
      <table className="usage-table">
        <thead>
          <tr>
            <th>{t("Time", "时间")}</th>
            <th>{t("Source", "来源")}</th>
            <th>{t("Provider", "供应商")}</th>
            <th>{t("Model", "模型")}</th>
            <th>{t("In", "输入")}</th>
            <th>{t("Cached", "缓存")}</th>
            <th>{t("Out", "输出")}</th>
            <th>{t("Duration", "耗时")}</th>
            <th>{t("Status", "状态")}</th>
          </tr>
        </thead>
        <tbody>
          {logs.map((record) => (
            <tr key={record.id}>
              <td>{formatTime(record.created_at, locale)}</td>
              <td>
                <strong>{sourceLabel(record.source, t)}</strong>
                <small>{record.operation}</small>
              </td>
              <td>{record.provider_name || record.provider_id}</td>
              <td className="mono">{record.model}</td>
              <td>{formatTokens(record.input_tokens)}</td>
              <td>{formatCacheDetail(record)}</td>
              <td>{formatTokens(record.output_tokens)}</td>
              <td>{formatDuration(record.duration_ms)}</td>
              <td>{statusLabel(effectiveStatus(record), t)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
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
