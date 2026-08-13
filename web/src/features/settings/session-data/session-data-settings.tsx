import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { CheckSquare, ChevronDown, ChevronRight, Database, Eraser, RefreshCw, Square, Trash2 } from "lucide-react";
import { api } from "../../../api/client";
import type { SessionDataSelection, SessionDataSummary } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import { formatSessionBytes, formatSessionDate } from "./session-data-format";
import "./session-data-settings.css";

/**
 * 渲染所有工作区的会话数据查看与清理界面。
 *
 * @returns 会话数据设置面板
 */
export function SessionDataSettings() {
  const { t, locale } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(() => new Set());
  const sessions = useQuery({
    queryKey: ["session-data"],
    queryFn: api.sessionData.list
  });
  const clearSession = useMutation({
    mutationFn: (items: SessionDataSelection[]) => api.sessionData.clearMany(items),
    onSuccess: async () => {
      setSelectedKeys(new Set());
      await queryClient.invalidateQueries({ queryKey: ["session-data"] });
    }
  });
  const deleteSession = useMutation({
    mutationFn: (id: string) => api.sessions.remove(id),    onSuccess: async (_result, id) => {
      if (expandedId?.endsWith(`/${id}`)) setExpandedId(null);
      setSelectedKeys((current) => {
        const next = new Set(current);
        for (const key of next) if (key.endsWith(`/${id}`)) next.delete(key);        return next;
      });
      await invalidateSessionQueries(queryClient, id);
    }
  });
  const deleteManySessions = useMutation({
    // 删除接口按会话逐个调用；串行执行避免同时写会话索引
    mutationFn: async (ids: string[]) => {
      for (const id of ids) await api.sessions.remove(id);
    },
    onSuccess: async () => {
      setExpandedId(null);
      setSelectedKeys(new Set());
      await queryClient.invalidateQueries({ queryKey: ["session-data"] });
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    }
  });
  const items = sessions.data ?? [];
  const groups = useMemo(() => groupByWorkspace(items), [items]);
  const selectedItems = items.filter((item) => selectedKeys.has(sessionKey(item)));
  const totalBytes = items.reduce((sum, session) => sum + session.total_bytes, 0);
  const allSelected = items.length > 0 && selectedItems.length === items.length;
  const busy = clearSession.isPending || deleteSession.isPending || deleteManySessions.isPending;
  const error = sessions.error ?? clearSession.error ?? deleteSession.error ?? deleteManySessions.error;

  /**
   * 切换单个会话的选择状态。
   *
   * @param session 目标会话摘要
   * @returns 无
   */
  const toggleSelected = (session: SessionDataSummary) => {
    const key = sessionKey(session);
    setSelectedKeys((current) => {
      const next = new Set(current);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  /**
   * 切换所有工作区会话的选择状态。
   *
   * @returns 无
   */
  const toggleAll = () => {
    setSelectedKeys(allSelected ? new Set() : new Set(items.map(sessionKey)));
  };

  /**
   * 确认并清空选中的会话数据。
   *
   * @param selected 待清空会话
   * @returns 无
   */
  const requestClear = async (selected: SessionDataSummary[]) => {
    if (selected.length === 0) return;
    const accepted = await confirm({
      title: t("Clear session data", "清空会话数据"),
      description: selected.length === 1
        ? t(
            `Clear all conversation and runtime data for “${selected[0].title}” while keeping the session entry?`,
            `清空“${selected[0].title}”的全部对话与运行数据，并保留会话条目？`
          )
        : t(
            `Clear all conversation and runtime data for ${selected.length} sessions across all workspaces?`,
            `清空所有工作区中 ${selected.length} 个会话的全部对话与运行数据？`
          ),
      confirmLabel: t("Clear data", "清空数据"),
      danger: true
    });
    if (accepted) {
      clearSession.mutate(selected.map(toSelection));
    }
  };

  /**
   * 确认并删除指定会话。
   *
   * @param session 目标会话摘要
   * @returns 无
   */
  const requestDelete = async (session: SessionDataSummary) => {
    const accepted = await confirm({
      title: t("Delete session", "删除会话"),
      description: t(
        `Permanently delete “${session.title}” and its session entry?`,
        `永久删除“${session.title}”及其会话条目？`
      ),
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (accepted) deleteSession.mutate(session.id);
  };

  /**
   * 确认并批量删除选中的会话。
   *
   * @param selected 待删除会话
   * @returns 无
   */
  const requestDeleteMany = async (selected: SessionDataSummary[]) => {
    if (selected.length === 0) return;
    const accepted = await confirm({
      title: t("Delete sessions", "删除会话"),
      description: t(
        `Permanently delete ${selected.length} sessions and their entries across all workspaces?`,
        `永久删除所有工作区中的 ${selected.length} 个会话及其会话条目？`
      ),
      confirmLabel: t("Delete", "删除"),
      danger: true
    });
    if (accepted) {
      deleteManySessions.mutate(selected.map((session) => session.id));
    }
  };

  return (
    <section className="session-data-settings">
      <header className="session-data-header">
        <div>
          <h2>{t("Session data", "会话数据")}</h2>
          <div className="session-data-total">
            <Database size={14} aria-hidden />
            <span>{t(`${items.length} sessions`, `${items.length} 个会话`)}</span>
            <span>{formatSessionBytes(totalBytes)}</span>
            <span>{t(`${groups.length} workspaces`, `${groups.length} 个工作区`)}</span>
          </div>
        </div>
        <div className="session-data-header-actions">
          <Button onClick={toggleAll} disabled={busy || items.length === 0} title={allSelected ? t("Clear selection", "取消全选") : t("Select all sessions", "全选会话")}>
            {allSelected ? <CheckSquare size={14} aria-hidden /> : <Square size={14} aria-hidden />}
            {allSelected ? t("Clear selection", "取消全选") : t("Select all", "全选")}
          </Button>
          {selectedItems.length > 0 && (
            <Button onClick={() => void requestClear(selectedItems)} disabled={busy} title={t("Clear selected session data", "清空选中会话数据")}>
              <Eraser size={14} aria-hidden />
              {t(`Clear ${selectedItems.length}`, `清空 ${selectedItems.length} 项`)}
            </Button>
          )}
          {selectedItems.length > 0 && (
            <Button onClick={() => void requestDeleteMany(selectedItems)} disabled={busy} title={t("Delete selected sessions", "删除选中会话")}>
              <Trash2 size={14} aria-hidden />
              {t(`Delete ${selectedItems.length}`, `删除 ${selectedItems.length} 项`)}
            </Button>
          )}
          <Button onClick={() => void sessions.refetch()} disabled={sessions.isFetching || busy} title={t("Refresh session data", "刷新会话数据")}>
            <RefreshCw size={14} aria-hidden />
            {t("Refresh", "刷新")}
          </Button>
        </div>
      </header>

      {sessions.isLoading && <div className="session-data-empty">{t("Loading session data", "正在读取会话数据")}</div>}
      {error && <div className="session-data-error">{error.message}</div>}
      {!sessions.isLoading && items.length === 0 && (
        <div className="session-data-empty">{t("No session data", "暂无会话数据")}</div>
      )}

      <div className="session-data-list">
        {groups.map((group) => (
          <section className="session-data-workspace" key={group.id}>
            <header className="session-data-workspace-header">
              <div>
                <strong>{group.name}</strong>
                <code title={group.path}>{group.path}</code>
              </div>
              <span>{t(`${group.items.length} sessions`, `${group.items.length} 个会话`)}</span>
            </header>
            <div className="session-data-workspace-list">
              {group.items.map((session) => (
                <SessionDataRow
                  key={sessionKey(session)}
                  session={session}
                  expanded={expandedId === sessionKey(session)}
                  selected={selectedKeys.has(sessionKey(session))}
                  busy={busy}
                  locale={locale}
                  onToggleExpanded={() => setExpandedId(expandedId === sessionKey(session) ? null : sessionKey(session))}
                  onToggleSelected={() => toggleSelected(session)}
                  onClear={() => void requestClear([session])}
                  onDelete={() => void requestDelete(session)}
                />
              ))}
            </div>
          </section>
        ))}
      </div>
    </section>
  );
}

type SessionDataRowProps = {
  session: SessionDataSummary;
  expanded: boolean;
  selected: boolean;
  busy: boolean;
  locale: string;
  onToggleExpanded: () => void;
  onToggleSelected: () => void;
  onClear: () => void;
  onDelete: () => void;
};

/**
 * 渲染单个会话行及其展开详情。
 *
 * @param props 会话行数据与交互回调
 * @returns 会话数据行
 */
function SessionDataRow({ session, expanded, selected, busy, locale, onToggleExpanded, onToggleSelected, onClear, onDelete }: SessionDataRowProps) {
  const { t } = useI18n();
  return (
    <article className={session.active ? "session-data-row active" : "session-data-row"}>
      <div className="session-data-row-main">
        <label className="session-data-select">
          <input type="checkbox" checked={selected} onChange={onToggleSelected} disabled={busy} aria-label={t(`Select ${session.title}`, `选择 ${session.title}`)} />
        </label>
        <Button
          className="session-data-expand"
          onClick={onToggleExpanded}
          aria-label={expanded ? t("Collapse details", "收起详情") : t("Expand details", "展开详情")}
          title={expanded ? t("Collapse details", "收起详情") : t("Expand details", "展开详情")}
        >
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </Button>
        <div className="session-data-identity">
          <div>
            <strong>{session.title}</strong>
            {session.active && <span className="session-data-active">{t("Active", "当前")}</span>}
          </div>
          <span>{formatSessionDate(session.updated_at, locale)}</span>
        </div>
        <dl className="session-data-metrics">
          <div><dt>{t("Size", "大小")}</dt><dd>{formatSessionBytes(session.total_bytes)}</dd></div>
          <div><dt>{t("Turns", "轮次")}</dt><dd>{metricValue(session.turn_count)}</dd></div>
          <div><dt>{t("Files", "文件")}</dt><dd>{session.file_count}</dd></div>
        </dl>
        <div className="session-data-actions">
          <Button onClick={onClear} disabled={busy} title={t("Clear session data", "清空会话数据")}>
            <Eraser size={14} aria-hidden />
            {t("Clear", "清空")}
          </Button>
          <Button variant="ghost-danger" onClick={onDelete} disabled={busy} title={t("Delete session", "删除会话")}>
            <Trash2 size={14} aria-hidden />
            {t("Delete", "删除")}
          </Button>
        </div>
      </div>
      {expanded && <SessionDataDetails session={session} />}
    </article>
  );
}

/**
 * 按工作区分组会话摘要。
 *
 * @param items 会话摘要
 * @returns 工作区分组
 */
function groupByWorkspace(items: SessionDataSummary[]): Array<{ id: string; name: string; path: string; items: SessionDataSummary[] }> {
  const groups = new Map<string, { id: string; name: string; path: string; items: SessionDataSummary[] }>();
  for (const item of items) {
    const current = groups.get(item.workspace_id) ?? { id: item.workspace_id, name: item.workspace_name, path: item.workspace_path, items: [] };
    current.items.push(item);
    groups.set(item.workspace_id, current);
  }
  return [...groups.values()];
}

/**
 * 生成跨工作区稳定选择键。
 *
 * @param session 会话摘要
 * @returns 选择键
 */
function sessionKey(session: SessionDataSummary): string {
  return `${session.workspace_id}/${session.id}`;
}

/**
 * 将会话摘要转换为批量清空请求项。
 *
 * @param session 会话摘要
 * @returns 清空请求项
 */
function toSelection(session: SessionDataSummary): SessionDataSelection {
  return { workspace_id: session.workspace_id, session_id: session.id };
}

/**
 * 渲染单个会话的详细状态与文件列表。
 *
 * @param props 会话数据摘要
 * @returns 会话数据详情
 */
function SessionDataDetails({ session }: { session: SessionDataSummary }) {
  const { t } = useI18n();
  return (
    <div className="session-data-details">
      <dl className="session-data-detail-metrics">
        <div><dt>{t("Branches", "分叉")}</dt><dd>{metricValue(session.branch_points)}</dd></div>
        <div><dt>{t("Loaded tools", "已加载工具")}</dt><dd>{metricValue(session.loaded_tool_count)}</dd></div>
        <div><dt>{t("Todos", "待办")}</dt><dd>{metricValue(session.todo_count)}</dd></div>
        <div><dt>{t("Goal", "目标")}</dt><dd>{session.has_goal == null ? "-" : session.has_goal ? t("Present", "存在") : t("None", "无")}</dd></div>
      </dl>
      {session.state_error && <div className="session-data-error">{session.state_error}</div>}
      <div className="session-data-table-wrap">
        <table className="session-data-table">
          <thead>
            <tr>
              <th>{t("Data item", "数据项")}</th>
              <th>{t("Type", "类型")}</th>
              <th>{t("Files", "文件")}</th>
              <th>{t("Size", "大小")}</th>
            </tr>
          </thead>
          <tbody>
            {session.items.map((item) => (
              <tr key={item.name}>
                <td>{item.name}</td>
                <td>{item.kind === "directory" ? t("Directory", "目录") : item.kind === "file" ? t("File", "文件") : t("Other", "其它")}</td>
                <td>{item.file_count}</td>
                <td>{formatSessionBytes(item.bytes)}</td>
              </tr>
            ))}
            {session.items.length === 0 && (
              <tr><td colSpan={4}>{t("No stored files", "没有已存储文件")}</td></tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

/**
 * 格式化可选指标。
 *
 * @param value 可选数值
 * @returns 数值文本；不可用时返回短横线
 */
function metricValue(value: number | null | undefined): string {
  return value == null ? "-" : String(value);
}

/**
 * 使会话数据变更同步到各个会话视图。
 *
 * @param queryClient React Query 客户端
 * @param sessionId 变更的会话标识
 * @returns 全部失效操作完成后的 Promise
 */
async function invalidateSessionQueries(queryClient: QueryClient, sessionId: string): Promise<void> {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: ["session-data"] }),
    queryClient.invalidateQueries({ queryKey: ["sessions"] }),
    queryClient.invalidateQueries({ queryKey: ["session-turn-tree", sessionId] }),
    queryClient.invalidateQueries({ queryKey: ["timeline", sessionId] }),
    queryClient.invalidateQueries({ queryKey: ["todos"] }),
    queryClient.invalidateQueries({ queryKey: ["goal", sessionId] }),
    queryClient.invalidateQueries({ queryKey: ["subagents"] }),
    queryClient.invalidateQueries({ queryKey: ["system-usage"] })
  ]);
}
