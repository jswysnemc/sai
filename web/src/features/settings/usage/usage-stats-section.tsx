import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, ChevronRight, RefreshCw, Trash2 } from "../../../shared/ui/icons";
import { useSearchParams } from "react-router-dom";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { ChoicePills, SettingsPanel, SwitchField } from "../kit";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { UsageGroupTable } from "./usage-group-table";
import { type UsageView } from "./usage-labels";
import { UsageLogsTable } from "./usage-logs-table";
import { UsageOverview } from "./usage-overview";
import { UsageStatsFilters, type UsageFilterState } from "./usage-stats-filters";
import { UsageSessionRanking } from "./usage-session-ranking";
import { UsageSessionLogDialog } from "./usage-session-log-dialog";
import type { UsageSessionStats, UsageSessionSort } from "../../../api/contracts/usage";
import "./usage-stats.css";

const LOG_PAGE_SIZE = 25;

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
  const confirm = useConfirm();
  const [params, setParams] = useSearchParams();
  const comparison = ["providers", "models", "sessions"].includes(params.get("view") ?? "") ? params.get("view")! : "providers";
  const [autoRefresh, setAutoRefresh] = useState(false);
  const view = (subview ?? "overview") as UsageView;
  const [filters, setFilters] = useState<UsageFilterState>(INITIAL_FILTERS);
  const [page, setPage] = useState(0);
  const [sessionSort, setSessionSort] = useState<UsageSessionSort>("total_tokens");
  const [sessionLimit, setSessionLimit] = useState(10);
  const [selectedSession, setSelectedSession] = useState<UsageSessionStats | null>(null);

  const stats = useQuery({
    queryKey: ["usage-stats", filters, page, sessionSort, sessionLimit],
    refetchInterval: autoRefresh && view === "logs" ? 5000 : false,
    queryFn: () =>
      api.usage.stats({
        range: filters.range,
        source: filters.source === "all" ? undefined : filters.source,
        status: filters.status === "all" ? undefined : filters.status,
        provider_search: filters.providerSearch.trim() || undefined,
        model_search: filters.modelSearch.trim() || undefined,
        limit: LOG_PAGE_SIZE,
        offset: page * LOG_PAGE_SIZE,
        session_sort: sessionSort,
        session_limit: sessionLimit,
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

  const workspaces = useQuery({ queryKey: ["workspaces"], queryFn: api.workspaces.list });
  const data = stats.data;
  const totalPages = Math.max(1, Math.ceil((data?.total_logs ?? 0) / LOG_PAGE_SIZE));

  return (
    <section className="usage-stats-section">
      <SettingsPanel title={t("Filters", "筛选条件")} description={t("Bill estimate folds cache discounts so you can compare with the vendor invoice. Provider-reported totals stay raw.", "账单估算已折算缓存折扣，便于对照供应商账单；接口上报量保持原始数值。")} actions={<>
        <Button variant="secondary" onClick={() => void stats.refetch()} disabled={stats.isFetching}><RefreshCw size={14} />{t("Refresh", "刷新")}</Button>
        <Button variant="ghost-danger" disabled={clear.isPending} onClick={() => void confirm({ title: t("Clear all usage logs?", "清空全部用量日志？"), description: t("This removes logs for every time range and cannot be undone.", "将删除全部时间范围的日志，操作无法恢复。"), confirmLabel: t("Clear all", "全部清空"), danger: true }).then((accepted) => { if (accepted) clear.mutate(); })}><Trash2 size={14} />{t("Clear", "清空")}</Button>
      </>}>
      <UsageStatsFilters value={filters} onChange={applyFilters} t={t} />
      </SettingsPanel>

      {stats.isLoading && <div className="usage-empty">{t("Loading usage", "正在读取用量")}</div>}
      {stats.error && <div className="usage-error">{stats.error.message}</div>}
      {clear.error && <div className="usage-error">{clear.error.message}</div>}

      {view === "breakdown" && <ChoicePills value={comparison} onChange={(next) => setParams((current) => { const updated = new URLSearchParams(current); updated.set("view", next); return updated; }, { replace: true })} ariaLabel={t("Compare by", "对比维度")} options={[{ value: "providers", label: t("Providers", "供应商") }, { value: "models", label: t("Models", "模型") }, { value: "sessions", label: t("Sessions", "会话") }]} />}
      {view === "logs" && <SwitchField label={t("Refresh every 5 seconds", "每 5 秒刷新")} checked={autoRefresh} onChange={setAutoRefresh} />}
      {data && view === "overview" && <UsageOverview data={data} t={t} locale={locale} ranking={<UsageSessionRanking rows={data.session_stats ?? []} total={data.total_sessions ?? 0} sort={sessionSort} limit={sessionLimit} onSortChange={setSessionSort} onLimitChange={setSessionLimit} workspaces={workspaces.data?.workspaces} onSelect={setSelectedSession} t={t} locale={locale} compact />} />}
      {data && view === "breakdown" && comparison === "sessions" && <UsageSessionRanking rows={data.session_stats ?? []} total={data.total_sessions ?? 0} sort={sessionSort} limit={sessionLimit} onSortChange={setSessionSort} onLimitChange={setSessionLimit} workspaces={workspaces.data?.workspaces} onSelect={setSelectedSession} t={t} locale={locale} />}
      {data && view === "breakdown" && comparison === "providers" && <UsageGroupTable rows={data.provider_stats} type="provider" t={t} locale={locale} />}
      {data && view === "breakdown" && comparison === "models" && <UsageGroupTable rows={data.model_stats} type="model" t={t} locale={locale} />}
      {data && view === "logs" && (
        <>
          <UsageLogsTable logs={data.logs} t={t} locale={locale} />
          <div className="usage-pager">
            <Button
              variant="secondary"
              disabled={page <= 0}
              onClick={() => setPage((value) => Math.max(0, value - 1))}
              aria-label={t("Previous page", "上一页")}
            >
              <ChevronLeft size={14} />
            </Button>
            <span>{page + 1} / {totalPages} · {data.total_logs}</span>
            <Button
              variant="secondary"
              disabled={page + 1 >= totalPages}
              onClick={() => setPage((value) => value + 1)}
              aria-label={t("Next page", "下一页")}
            >
              <ChevronRight size={14} />
            </Button>
          </div>
        </>
      )}
      <UsageSessionLogDialog session={selectedSession} filters={filters} onClose={() => setSelectedSession(null)} />
    </section>
  );
}
