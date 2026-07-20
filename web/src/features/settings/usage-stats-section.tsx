import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, RefreshCw, Trash2 } from "lucide-react";
import { api } from "../../api/client";
import type {
  UsageGroupStats,
  UsageRange,
  UsageRecord,
  UsageStatsResponse,
  UsageTrendPoint,
} from "../../api/contracts";
import { useI18n } from "../i18n/use-i18n";
import "./usage-stats-section.css";

const LOG_PAGE_SIZE = 15;
const RANGES: UsageRange[] = ["today", "1d", "7d", "30d", "90d", "all"];
const SOURCES = ["all", "chat", "compaction", "session_memory"];
const STATUSES = ["all", "success", "error", "missing_usage"];

/**
 * 设置页用量统计：汇总、趋势、模型分布与请求日志。
 *
 * @returns 用量统计面板
 */
export function UsageStatsSection() {
  const { t, locale } = useI18n();
  const queryClient = useQueryClient();
  const [range, setRange] = useState<UsageRange>("7d");
  const [source, setSource] = useState("all");
  const [status, setStatus] = useState("all");
  const [providerSearch, setProviderSearch] = useState("");
  const [modelSearch, setModelSearch] = useState("");
  const [view, setView] = useState<"overview" | "providers" | "models" | "logs">("overview");
  const [page, setPage] = useState(0);

  const stats = useQuery({
    queryKey: ["usage-stats", range, source, status, providerSearch, modelSearch, page],
    queryFn: () =>
      api.usage.stats({
        range,
        source: source === "all" ? undefined : source,
        status: status === "all" ? undefined : status,
        provider_search: providerSearch.trim() || undefined,
        model_search: modelSearch.trim() || undefined,
        limit: LOG_PAGE_SIZE,
        offset: page * LOG_PAGE_SIZE,
      }),
  });

  const clear = useMutation({
    mutationFn: () => api.usage.clear(),
    onSuccess: async () => {
      setPage(0);
      await queryClient.invalidateQueries({ queryKey: ["usage-stats"] });
    },
  });

  const data = stats.data;
  const totalPages = Math.max(1, Math.ceil((data?.total_logs ?? 0) / LOG_PAGE_SIZE));

  return (
    <section className="usage-stats-section">
      <header className="usage-stats-header">
        <div>
          <h2>{t("Usage statistics", "用量统计")}</h2>
          <p>{t("Provider-reported tokens across chat and auxiliary calls.", "汇总 Chat 与辅助调用的 Provider Token 用量。")}</p>
        </div>
        <div className="usage-stats-actions">
          <button type="button" className="usage-btn" onClick={() => void stats.refetch()} disabled={stats.isFetching}>
            <RefreshCw size={14} />
            {t("Refresh", "刷新")}
          </button>
          <button
            type="button"
            className="usage-btn danger"
            onClick={() => clear.mutate()}
            disabled={clear.isPending}
            title={t("Clear all usage logs", "清空全部用量日志")}
          >
            <Trash2 size={14} />
            {t("Clear", "清空")}
          </button>
        </div>
      </header>

      <div className="usage-filters">
        <label>
          <span>{t("Range", "时间范围")}</span>
          <select value={range} onChange={(event) => { setRange(event.target.value as UsageRange); setPage(0); }}>
            {RANGES.map((item) => (
              <option key={item} value={item}>{rangeLabel(item, t)}</option>
            ))}
          </select>
        </label>
        <label>
          <span>{t("Source", "来源")}</span>
          <select value={source} onChange={(event) => { setSource(event.target.value); setPage(0); }}>
            {SOURCES.map((item) => (
              <option key={item} value={item}>{sourceLabel(item, t)}</option>
            ))}
          </select>
        </label>
        <label>
          <span>{t("Status", "状态")}</span>
          <select value={status} onChange={(event) => { setStatus(event.target.value); setPage(0); }}>
            {STATUSES.map((item) => (
              <option key={item} value={item}>{statusLabel(item, t)}</option>
            ))}
          </select>
        </label>
        <label>
          <span>{t("Provider", "供应商")}</span>
          <input value={providerSearch} onChange={(event) => { setProviderSearch(event.target.value); setPage(0); }} placeholder={t("Search provider", "搜索供应商")} />
        </label>
        <label>
          <span>{t("Model", "模型")}</span>
          <input value={modelSearch} onChange={(event) => { setModelSearch(event.target.value); setPage(0); }} placeholder={t("Search model", "搜索模型")} />
        </label>
      </div>

      <div className="usage-tabs">
        {(["overview", "providers", "models", "logs"] as const).map((item) => (
          <button key={item} type="button" className={view === item ? "active" : ""} onClick={() => setView(item)}>
            {viewLabel(item, t)}
          </button>
        ))}
      </div>

      {stats.isLoading && <div className="usage-empty">{t("Loading usage", "正在读取用量")}</div>}
      {stats.error && <div className="usage-error">{stats.error.message}</div>}
      {clear.error && <div className="usage-error">{clear.error.message}</div>}

      {data && view === "overview" && <Overview data={data} t={t} locale={locale} />}
      {data && view === "providers" && <GroupTable rows={data.provider_stats} type="provider" t={t} locale={locale} />}
      {data && view === "models" && <GroupTable rows={data.model_stats} type="model" t={t} locale={locale} />}
      {data && view === "logs" && (
        <>
          <LogsTable logs={data.logs} t={t} locale={locale} />
          <div className="usage-pager">
            <button type="button" disabled={page <= 0} onClick={() => setPage((value) => Math.max(0, value - 1))}>
              <ChevronLeft size={14} />
            </button>
            <span>{page + 1} / {totalPages} · {data.total_logs}</span>
            <button type="button" disabled={page + 1 >= totalPages} onClick={() => setPage((value) => value + 1)}>
              <ChevronRight size={14} />
            </button>
          </div>
        </>
      )}
    </section>
  );
}

function Overview({
  data,
  t,
  locale,
}: {
  data: UsageStatsResponse;
  t: (en: string, zh: string) => string;
  locale: "en-US" | "zh-CN";
}) {
  const summary = data.summary;
  return (
    <div className="usage-overview">
      <div className="usage-summary-grid">
        <SummaryTile label={t("Requests", "请求数")} value={formatCount(summary.total_requests)} sub={`${formatCount(summary.successful_requests)} ${t("ok", "成功")}`} />
        <SummaryTile label={t("Total tokens", "总 Token")} value={formatTokens(summary.total_tokens)} sub={`${formatTokens(summary.input_tokens)} / ${formatTokens(summary.output_tokens)}`} />
        <SummaryTile label={t("Avg duration", "平均耗时")} value={formatDuration(summary.average_duration_ms)} sub={`${formatCount(summary.provider_reported_requests)} ${t("reported", "有用量")}`} />
        <SummaryTile label={t("Missing usage", "无用量")} value={formatCount(summary.missing_usage_requests)} sub={`${formatCount(summary.failed_requests)} ${t("failed", "失败")}`} />
      </div>
      <div className="usage-panel">
        <h3>{t("Token trend", "Token 趋势")}</h3>
        <TrendChart points={data.trend} t={t} />
      </div>
      <div className="usage-panel">
        <h3>{t("Model distribution", "模型分布")}</h3>
        <ModelDonut rows={data.model_stats} t={t} />
      </div>
      <div className="usage-panel">
        <h3>{t("Top models", "模型排行")}</h3>
        <GroupTable rows={data.model_stats.slice(0, 8)} type="model" t={t} locale={locale} compact />
      </div>
    </div>
  );
}

function SummaryTile({ label, value, sub }: { label: string; value: string; sub?: string }) {
  return (
    <div className="usage-summary-tile">
      <small>{label}</small>
      <strong>{value}</strong>
      {sub && <i>{sub}</i>}
    </div>
  );
}

function TrendChart({ points, t }: { points: UsageTrendPoint[]; t: (en: string, zh: string) => string }) {
  const geom = useMemo(() => {
    const width = 640;
    const height = 180;
    const padL = 44;
    const padR = 16;
    const padT = 10;
    const padB = 20;
    const maxTokens = Math.max(1, ...points.map((point) => point.total_tokens));
    const step = points.length > 1 ? (width - padL - padR) / (points.length - 1) : 0;
    const plotH = height - padT - padB;
    const singleX = padL + (width - padL - padR) / 2;
    const x = (index: number) => (points.length > 1 ? padL + step * index : singleX);
    const y = (value: number) => padT + plotH - (value / maxTokens) * plotH;
    const path = points
      .map((point, index) => `${index === 0 ? "M" : "L"} ${x(index).toFixed(1)} ${y(point.total_tokens).toFixed(1)}`)
      .join(" ");
    return { width, height, padL, padR, padT, padB, maxTokens, plotH, x, y, path };
  }, [points]);

  if (points.length === 0) {
    return <div className="usage-empty">{t("No trend data", "暂无趋势数据")}</div>;
  }

  return (
    <svg viewBox={`0 0 ${geom.width} ${geom.height}`} className="usage-trend-svg" role="img" aria-label="token trend">
      {[0, 0.5, 1].map((fraction) => {
        const y = geom.padT + geom.plotH - fraction * geom.plotH;
        return (
          <g key={fraction}>
            <line x1={geom.padL} y1={y} x2={geom.width - geom.padR} y2={y} className="usage-grid" />
            <text x={geom.padL - 6} y={y + 3.5} textAnchor="end" className="usage-axis">
              {formatTokens(geom.maxTokens * fraction)}
            </text>
          </g>
        );
      })}
      {geom.path && <path d={geom.path} className="usage-trend-line" fill="none" />}
      {points.map((point, index) => (
        <circle key={point.date} cx={geom.x(index)} cy={geom.y(point.total_tokens)} r="2.5" className="usage-trend-dot" />
      ))}
      <text x={geom.padL} y={geom.height - 4} className="usage-axis">{points[0]?.label}</text>
      {points.length > 1 && (
        <text x={geom.width - geom.padR} y={geom.height - 4} textAnchor="end" className="usage-axis">
          {points[points.length - 1]?.label}
        </text>
      )}
    </svg>
  );
}

function ModelDonut({ rows, t }: { rows: UsageGroupStats[]; t: (en: string, zh: string) => string }) {
  const colors = ["#2a78d6", "#1baf7a", "#eda100", "#4a3aa7", "#e34948", "#0891b2", "#a8a29e"];
  const sorted = rows.filter((row) => row.total_tokens > 0).slice(0, 6);
  const total = sorted.reduce((sum, row) => sum + row.total_tokens, 0);
  if (total <= 0) {
    return <div className="usage-empty">{t("No model data", "暂无模型数据")}</div>;
  }
  let angle = -Math.PI / 2;
  const arcs = sorted.map((row, index) => {
    const sweep = (row.total_tokens / total) * Math.PI * 2;
    const start = angle;
    angle += sweep;
    return { row, color: colors[index % colors.length], path: donutPath(100, 100, 90, 54, start, angle) };
  });
  return (
    <div className="usage-donut-wrap">
      <svg viewBox="0 0 200 200" className="usage-donut" role="img" aria-label="model distribution">
        {arcs.map((arc) => (
          <path key={arc.row.id} d={arc.path} fill={arc.color} />
        ))}
        <text x="100" y="96" textAnchor="middle" className="usage-donut-label">{t("Total", "总计")}</text>
        <text x="100" y="114" textAnchor="middle" className="usage-donut-value">{formatTokens(total)}</text>
      </svg>
      <ul className="usage-donut-legend">
        {sorted.map((row, index) => (
          <li key={row.id}>
            <i style={{ background: colors[index % colors.length] }} />
            <span>{row.label}</span>
            <strong>{formatTokens(row.total_tokens)}</strong>
          </li>
        ))}
      </ul>
    </div>
  );
}

function donutPath(cx: number, cy: number, rOuter: number, rInner: number, start: number, end: number): string {
  const full = end - start >= Math.PI * 2 - 1e-4;
  const p = (r: number, a: number) => `${cx + r * Math.cos(a)} ${cy + r * Math.sin(a)}`;
  if (full) {
    return [
      `M ${cx} ${cy - rOuter}`,
      `A ${rOuter} ${rOuter} 0 1 1 ${cx} ${cy + rOuter}`,
      `A ${rOuter} ${rOuter} 0 1 1 ${cx} ${cy - rOuter}`,
      `M ${cx} ${cy - rInner}`,
      `A ${rInner} ${rInner} 0 1 0 ${cx} ${cy + rInner}`,
      `A ${rInner} ${rInner} 0 1 0 ${cx} ${cy - rInner}`,
      "Z",
    ].join(" ");
  }
  const large = end - start > Math.PI ? 1 : 0;
  return [
    `M ${p(rOuter, start)}`,
    `A ${rOuter} ${rOuter} 0 ${large} 1 ${p(rOuter, end)}`,
    `L ${p(rInner, end)}`,
    `A ${rInner} ${rInner} 0 ${large} 0 ${p(rInner, start)}`,
    "Z",
  ].join(" ");
}

function GroupTable({
  rows,
  type,
  t,
  locale,
  compact,
}: {
  rows: UsageGroupStats[];
  type: "provider" | "model";
  t: (en: string, zh: string) => string;
  locale: "en-US" | "zh-CN";
  compact?: boolean;
}) {
  if (rows.length === 0) {
    return <div className="usage-empty">{t("No usage data", "暂无统计数据")}</div>;
  }
  return (
    <div className="usage-table-wrap">
      <table className="usage-table">
        <thead>
          <tr>
            <th>{type === "provider" ? "Provider" : t("Model", "模型")}</th>
            <th>{t("Req", "请求")}</th>
            <th>{t("Success", "成功率")}</th>
            <th>Token</th>
            <th>{t("In/Out", "输入/输出")}</th>
            {!compact && <th>{t("Avg", "均耗时")}</th>}
            {!compact && <th>{t("Last", "最近")}</th>}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => {
            const rate = row.request_count > 0 ? row.success_count / row.request_count : 0;
            return (
              <tr key={row.id}>
                <td>
                  <strong>{row.label}</strong>
                  {type === "model" && row.provider_name && <small>{row.provider_name}</small>}
                </td>
                <td>{formatCount(row.request_count)}</td>
                <td>{Math.round(rate * 100)}%</td>
                <td>{formatTokens(row.total_tokens)}</td>
                <td>{formatTokens(row.input_tokens)} / {formatTokens(row.output_tokens)}</td>
                {!compact && <td>{formatDuration(row.average_duration_ms)}</td>}
                {!compact && <td>{formatTime(row.last_used_at, locale)}</td>}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function LogsTable({
  logs,
  t,
  locale,
}: {
  logs: UsageRecord[];
  t: (en: string, zh: string) => string;
  locale: "en-US" | "zh-CN";
}) {
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
            <th>Provider</th>
            <th>Model</th>
            <th>{t("In", "输入")}</th>
            <th>{t("Out", "输出")}</th>
            <th>Token</th>
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
              <td>{formatTokens(record.output_tokens)}</td>
              <td>{formatTokens(record.total_tokens ?? ((record.input_tokens ?? 0) + (record.output_tokens ?? 0)))}</td>
              <td>{formatDuration(record.duration_ms)}</td>
              <td>{statusLabel(record.status === "success" && record.usage_source === "missing" ? "missing_usage" : record.status, t)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function formatCount(value?: number | null) {
  if (!value || !Number.isFinite(value)) return "0";
  return Math.round(value).toLocaleString();
}

function formatTokens(value?: number | null) {
  const n = Number(value ?? 0);
  if (!Number.isFinite(n) || n <= 0) return "0";
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(n >= 10_000_000 ? 1 : 2)}M`;
  if (n >= 10_000) return `${(n / 1_000).toFixed(1)}K`;
  return Math.round(n).toLocaleString();
}

function formatDuration(ms?: number | null) {
  const n = Number(ms ?? 0);
  if (!Number.isFinite(n) || n <= 0) return "--";
  if (n >= 1000) return `${(n / 1000).toFixed(1)}s`;
  return `${Math.round(n)}ms`;
}

function formatTime(seconds?: number | null, locale: "en-US" | "zh-CN" = "zh-CN") {
  if (!seconds) return "--";
  return new Date(seconds * 1000).toLocaleString(locale, {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function rangeLabel(range: string, t: (en: string, zh: string) => string) {
  const map: Record<string, [string, string]> = {
    today: ["Today", "今天"],
    "1d": ["Last 24h", "近 24 小时"],
    "7d": ["Last 7 days", "近 7 天"],
    "30d": ["Last 30 days", "近 30 天"],
    "90d": ["Last 90 days", "近 90 天"],
    all: ["All time", "全部"],
  };
  const pair = map[range] ?? [range, range];
  return t(pair[0], pair[1]);
}

function sourceLabel(source: string, t: (en: string, zh: string) => string) {
  const map: Record<string, [string, string]> = {
    all: ["All sources", "全部来源"],
    chat: ["Chat", "对话"],
    compaction: ["Compaction", "上下文压缩"],
    session_memory: ["Session memory", "会话记忆"],
  };
  const pair = map[source] ?? [source, source];
  return t(pair[0], pair[1]);
}

function statusLabel(status: string, t: (en: string, zh: string) => string) {
  const map: Record<string, [string, string]> = {
    all: ["All statuses", "全部状态"],
    success: ["Success", "成功"],
    error: ["Error", "失败"],
    missing_usage: ["No usage", "无用量"],
  };
  const pair = map[status] ?? [status, status];
  return t(pair[0], pair[1]);
}

function viewLabel(view: string, t: (en: string, zh: string) => string) {
  const map: Record<string, [string, string]> = {
    overview: ["Overview", "总览"],
    providers: ["Providers", "供应商"],
    models: ["Models", "模型"],
    logs: ["Logs", "日志"],
  };
  const pair = map[view] ?? [view, view];
  return t(pair[0], pair[1]);
}
