import type { UsageStatsQuery, UsageStatsResponse } from "./contracts/usage";
import { apiRequest } from "./api-request";

/** 用量统计请求，独立于其他业务接口维护筛选与排行参数。 */
export const usageApi = {
  /**
   * 【用量统计】【查询】发送共用筛选条件、日志分页与会话排行参数。
   * @param query 时间、来源、状态、分页和排行设置
   * @returns 统计响应
   */
  stats: (query: UsageStatsQuery = {}) => {
    const params = new URLSearchParams();
    if (query.range) params.set("range", query.range);
    if (query.source) params.set("source", query.source);
    if (query.status) params.set("status", query.status);
    if (query.provider_search) params.set("provider_search", query.provider_search);
    if (query.model_search) params.set("model_search", query.model_search);
    if (query.limit != null) params.set("limit", String(query.limit));
    if (query.offset != null) params.set("offset", String(query.offset));
    if (query.session_sort) params.set("session_sort", query.session_sort);
    if (query.session_limit != null) params.set("session_limit", String(query.session_limit));
    const suffix = params.size > 0 ? `?${params.toString()}` : "";
    return apiRequest<UsageStatsResponse>(`/api/usage/stats${suffix}`);
  },
  /** 【用量统计】【清空日志】删除已有用量记录，返回操作结果。 */
  clear: () => apiRequest<{ ok: boolean }>("/api/usage/logs", { method: "DELETE" })
};
