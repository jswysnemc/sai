import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, RefreshCw, Trash2 } from "lucide-react";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { UsageGroupTable } from "./usage-group-table";
import { type UsageView } from "./usage-labels";
import { UsageLogsTable } from "./usage-logs-table";
import { UsageOverview } from "./usage-overview";
import { UsageStatsFilters, type UsageFilterState } from "./usage-stats-filters";
import "./usage-stats.css";

const LOG_PAGE_SIZE = 15;

const INITIAL_FILTERS: UsageFilterState = {
  range: "7d",
  source: "all",
  status: "all",
  providerSearch: "",
  modelSearch: "",
};

/**
 * 设置页用量统计面板：筛选、汇总、分组与请求日志。
 *
 * 视图切换来自路由子页（/settings/usage/:subview），刷新后停留原视图。
 *
 * @param props subview 为当前子页
 * @returns 用量统计面板
 */
export function UsageStatsSection({ subview }: { subview?: string }) {
  const { t, locale } = useI18n();
  const queryClient = useQueryClient();
  const view = (subview ?? "overview") as UsageView;
  const [filters, setFilters] = useState<UsageFilterState>(INITIAL_FILTERS);
  const [page, setPage] = useState(0);

  const stats = useQuery({
    queryKey: ["usage-stats", filters, page],
    queryFn: () =>
      api.usage.stats({
        range: filters.range,
        source: filters.source === "all" ? undefined : filters.source,
        status: filters.status === "all" ? undefined : filters.status,
        provider_search: filters.providerSearch.trim() || undefined,
        model_search: filters.modelSearch.trim() || undefined,
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

  /**
   * 更新筛选条件并回到首页。
   *
   * @param next 变更的筛选字段
   * @returns 无
   */
  const applyFilters = (next: Partial<UsageFilterState>) => {
    setFilters((current) => ({ ...current, ...next }));
    setPage(0);
  };

  const data = stats.data;
  const totalPages = Math.max(1, Math.ceil((data?.total_logs ?? 0) / LOG_PAGE_SIZE));

  return (
    <section className="usage-stats-section">
      <header className="usage-stats-header">
        <div>
          <h2>{t("Usage statistics", "用量统计")}</h2>
          <p>
            {t(
              "Provider-reported tokens across chat and auxiliary calls, with cache-adjusted billable totals.",
              "汇总 Chat 与辅助调用的 Token 用量，并按缓存计价折算出可与账单对比的口径。"
            )}
          </p>
        </div>
        <div className="usage-stats-actions">
          <Button className="settings-secondary" onClick={() => void stats.refetch()} disabled={stats.isFetching}>
            <RefreshCw size={14} />
            {t("Refresh", "刷新")}
          </Button>
          <Button
            variant="danger"
            onClick={() => clear.mutate()}
            disabled={clear.isPending}
            title={t("Clear all usage logs", "清空全部用量日志")}
          >
            <Trash2 size={14} />
            {t("Clear", "清空")}
          </Button>
        </div>
      </header>

      <UsageStatsFilters value={filters} onChange={applyFilters} t={t} />

      {stats.isLoading && <div className="usage-empty">{t("Loading usage", "正在读取用量")}</div>}
      {stats.error && <div className="usage-error">{stats.error.message}</div>}
      {clear.error && <div className="usage-error">{clear.error.message}</div>}

      {data && view === "overview" && <UsageOverview data={data} t={t} locale={locale} />}
      {data && view === "providers" && <UsageGroupTable rows={data.provider_stats} type="provider" t={t} locale={locale} />}
      {data && view === "models" && <UsageGroupTable rows={data.model_stats} type="model" t={t} locale={locale} />}
      {data && view === "logs" && (
        <>
          <UsageLogsTable logs={data.logs} t={t} locale={locale} />
          <div className="usage-pager">
            <Button
              className="settings-secondary"
              disabled={page <= 0}
              onClick={() => setPage((value) => Math.max(0, value - 1))}
              aria-label={t("Previous page", "上一页")}
            >
              <ChevronLeft size={14} />
            </Button>
            <span>{page + 1} / {totalPages} · {data.total_logs}</span>
            <Button
              className="settings-secondary"
              disabled={page + 1 >= totalPages}
              onClick={() => setPage((value) => value + 1)}
              aria-label={t("Next page", "下一页")}
            >
              <ChevronRight size={14} />
            </Button>
          </div>
        </>
      )}
    </section>
  );
}
