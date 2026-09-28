import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import type { UsageSessionStats } from "../../../api/contracts/usage";
import { api } from "../../../api/client";
import { Button } from "../../../shared/ui/button/button";
import { Modal } from "../../../shared/ui/dialog/modal";
import { useI18n } from "../../i18n/use-i18n";
import { UsageLogsTable } from "./usage-logs-table";
import type { UsageFilterState } from "./usage-stats-filters";

type Props = { session: UsageSessionStats | null; filters: UsageFilterState; onClose: () => void };

/**
 * 【用量】【会话日志】按工作区与会话精确查询完整分页日志。
 * @param props 会话、当前筛选条件与关闭回调
 * @returns 日志弹窗
 */
export function UsageSessionLogDialog({ session, filters, onClose }: Props) {
  const { t, locale } = useI18n();
  const [page, setPage] = useState(0);
  useEffect(() => { setPage(0); }, [session?.session_id, session?.workspace_id]);
  const query = useQuery({
    queryKey: ["usage-session-logs", session?.workspace_id, session?.session_id, filters, page],
    enabled: Boolean(session),
    queryFn: () => api.usage.stats({ range: filters.range, source: filters.source, status: filters.status, provider_search: filters.providerSearch, model_search: filters.modelSearch, session_id: session!.session_id, workspace_id: session!.workspace_id ?? "", offset: page * 25, limit: 25 })
  });
  const pages = Math.max(1, Math.ceil((query.data?.total_logs ?? 0) / 25));
  return <Modal open={Boolean(session)} title={session?.title || session?.session_id || t("Session logs", "会话日志")} onClose={onClose} className="usage-session-log-dialog">
    {query.isLoading && <p>{t("Loading logs", "正在读取日志")}</p>}
    {query.error && <div className="settings-inline-error">{query.error.message}</div>}
    {query.data && <UsageLogsTable logs={query.data.logs} t={t} locale={locale} />}
    <div className="flex items-center justify-end gap-2 pt-3 text-xs">
      <Button variant="secondary" disabled={!page || query.isFetching} onClick={() => setPage(page - 1)}>{t("Previous", "上一页")}</Button>
      <span>{page + 1} / {pages}</span>
      <Button variant="secondary" disabled={page + 1 >= pages || query.isFetching} onClick={() => setPage(page + 1)}>{t("Next", "下一页")}</Button>
    </div>
  </Modal>;
}
