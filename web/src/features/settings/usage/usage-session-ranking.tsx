import type { UsageSessionSort, UsageSessionStats } from "../../../api/contracts/usage";
import { Select } from "../../../shared/ui/select/select";
import { formatCount, formatTime, formatTokens } from "./usage-format";
import type { Translate } from "./usage-labels";
import "./usage-session-ranking.css";

type UsageSessionRankingProps = {
  rows: UsageSessionStats[];
  total: number;
  sort: UsageSessionSort;
  limit: number;
  onSortChange: (sort: UsageSessionSort) => void;
  onLimitChange: (limit: number) => void;
  t: Translate;
  locale: "en-US" | "zh-CN";
};

/**
 * 【用量统计】【会话排行】展示当前筛选范围内高消耗会话，并支持切换排名指标。
 * @param props 会话统计、排序、数量、更新回调与语言
 * @returns 排行表及加载完成后的空态
 */
export function UsageSessionRanking({ rows, total, sort, limit, onSortChange, onLimitChange, t, locale }: UsageSessionRankingProps) {
  return (
    <section className="usage-panel usage-session-ranking" aria-label={t("Top-consuming sessions", "高消耗会话")}>
      <header className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div className="min-w-0">
          <h3>{t("Top-consuming sessions", "高消耗会话")}</h3>
          <p className="usage-session-caption">
            {t(`${formatCount(total)} sessions in the selected range`, `当前筛选范围共 ${formatCount(total)} 个会话`)}
          </p>
        </div>
        <div className="grid grid-cols-[minmax(0,1fr)_6rem] gap-2 sm:shrink-0 sm:grid-cols-[11rem_6rem]">
          <Select<UsageSessionSort>
            value={sort}
            ariaLabel={t("Rank sessions by", "会话排名指标")}
            options={[
              { value: "total_tokens", label: t("Total tokens", "总 Token") },
              { value: "billable_tokens", label: t("Billable tokens", "等效计费 Token") },
              { value: "requests", label: t("Requests", "请求次数") },
            ]}
            onChange={onSortChange}
          />
          <Select
            value={String(limit)}
            ariaLabel={t("Ranking size", "排行数量")}
            options={[10, 20, 50].map((value) => ({ value: String(value), label: t(`Top ${value}`, `前 ${value} 名`) }))}
            onChange={(value) => onLimitChange(Number(value))}
          />
        </div>
      </header>
      {rows.length === 0 ? (
        <div className="usage-empty">{t("No session usage in this range", "当前筛选范围内暂无会话用量")}</div>
      ) : (
        <div className="usage-table-wrap">
          <table className="usage-table usage-session-table table-fixed sm:table-auto">
            <thead>
              <tr>
                <th scope="col" className="usage-session-rank">{t("Rank", "排名")}</th>
                <th scope="col" className="usage-session-name">{t("Session", "会话")}</th>
                <th scope="col" className="usage-session-primary sm:hidden">
                  {sort === "requests" ? t("Requests", "请求") : sort === "billable_tokens" ? t("Billable tokens", "计费 Token") : t("Total tokens", "总 Token")}
                </th>
                <th scope="col" className="hidden sm:table-cell">{t("Total tokens", "总 Token")}</th>
                <th scope="col" className="hidden sm:table-cell">{t("Billable tokens", "等效计费 Token")}</th>
                <th scope="col" className="hidden sm:table-cell">{t("Requests", "请求")}</th>
                <th scope="col" className="hidden md:table-cell">{t("Input / output", "输入 / 输出")}</th>
                <th scope="col" className="hidden lg:table-cell">{t("Last used", "最近调用")}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((row, index) => (
                <tr key={`${row.workspace_id ?? "unknown"}:${row.session_id}`}>
                  <td className="usage-session-rank">{index + 1}</td>
                  <td className="usage-session-name">
                    <strong title={row.title || row.session_id}>{row.title || row.session_id}</strong>
                    <small title={row.session_id}>{row.session_id}</small>
                    {row.workspace_id && <span className="hidden sm:block"><small title={row.workspace_id}>{t("Workspace", "工作区")} · {row.workspace_id}</small></span>}
                  </td>
                  <td className="usage-cell-accent usage-session-primary sm:hidden">
                    {sort === "requests" ? formatCount(row.total_requests) : formatTokens(sort === "billable_tokens" ? row.billable_total_tokens : row.total_tokens)}
                  </td>
                  <td className={`hidden sm:table-cell${sort === "total_tokens" ? " usage-cell-accent" : ""}`}>{formatTokens(row.total_tokens)}</td>
                  <td className={`hidden sm:table-cell${sort === "billable_tokens" ? " usage-cell-accent" : ""}`}>{formatTokens(row.billable_total_tokens)}</td>
                  <td className={`hidden sm:table-cell${sort === "requests" ? " usage-cell-accent" : ""}`}>
                    {formatCount(row.total_requests)}
                    {row.missing_usage_requests > 0 && <small>{t(`${formatCount(row.missing_usage_requests)} without token usage`, `${formatCount(row.missing_usage_requests)} 次未上报用量`)}</small>}
                  </td>
                  <td className="hidden md:table-cell">{formatTokens(row.input_tokens)} / {formatTokens(row.output_tokens)}</td>
                  <td className="hidden lg:table-cell">{formatTime(row.last_used_at, locale)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
