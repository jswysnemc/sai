import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { Eraser, RefreshCw, Trash2 } from "../../../shared/ui/icons";
import { api } from "../../../api/client";
import type { SessionDataSelection, SessionDataSummary } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import { Modal } from "../../../shared/ui/dialog/modal";
import { FieldGrid, SettingsField, SettingsPanel, SkSelect, SkTextInput } from "../kit";
import { filterSessionData, isIdleSession, sessionKey, toSelection } from "./session-data-selection";
import { SessionDataDetails } from "./session-data-details";
import { SessionDataTable } from "./session-data-table";
import { formatSessionBytes } from "./session-data-format";

/**
 * 渲染所有工作区的会话数据查看与清理界面。
 *
 * @returns 会话数据设置面板
 */
export function SessionDataSettings() {
  const { t } = useI18n();
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [query, setQuery] = useState("");
  const [workspaceId, setWorkspaceId] = useState("");
  const [minimum, setMinimum] = useState("0");
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(() => new Set());
  const [deleteWarning, setDeleteWarning] = useState<string | null>(null);
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
  const deleteSessions = useMutation({
    // 删除按工作区定位：此前复用只带会话 ID 的接口，删别的工作区的会话会静默失败
    mutationFn: (targets: SessionDataSummary[]) =>
      api.sessionData.deleteMany(targets.map(toSelection)),
    onSuccess: async (result) => {
      // 后端会报告索引里找不到的会话，必须显式提示，不能当作删除成功
      setDeleteWarning(
        result.missing_ids.length > 0
          ? t(
              `${result.missing_ids.length} sessions were not found and remain in the list`,
              `${result.missing_ids.length} 个会话未找到，仍保留在列表中`
            )
          : null
      );
      setExpandedId(null);
      setSelectedKeys(new Set());
      await Promise.all(result.deleted_ids.map((id) => invalidateSessionQueries(queryClient, id)));
      await queryClient.invalidateQueries({ queryKey: ["session-data"] });
      await queryClient.invalidateQueries({ queryKey: ["sessions"] });
    }
  });
  const items = sessions.data ?? [];
  const visible = useMemo(() => filterSessionData(items, query, workspaceId, Number(minimum)), [items, query, workspaceId, minimum]);
  const workspaces = [...new Map(items.map((item) => [item.workspace_id, { value: item.workspace_id, label: item.workspace_name }])).values()];
  const idle = visible.filter(isIdleSession);
  const selectedItems = items.filter((item) => isIdleSession(item) && selectedKeys.has(sessionKey(item)));
  const totalBytes = items.reduce((sum, session) => sum + session.total_bytes, 0);
  const busy = clearSession.isPending || deleteSessions.isPending;
  const error = sessions.error ?? clearSession.error ?? deleteSessions.error;

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
    if (accepted) deleteSessions.mutate([session]);
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
      deleteSessions.mutate(selected);
    }
  };

  const detailed = items.find((item) => sessionKey(item) === expandedId);
  return <SettingsPanel title={t("Stored sessions", "已存会话")} description={t(`${items.length} sessions · ${formatSessionBytes(totalBytes)}`, `${items.length} 个会话 · ${formatSessionBytes(totalBytes)}`)} actions={<>
    <Button variant="secondary" disabled={busy || !idle.length} onClick={() => setSelectedKeys(new Set(idle.map(sessionKey)))}>{t("Select displayed idle sessions", "选择展示的空闲会话")}</Button>
    {selectedItems.length > 0 && <>
      <Button variant="secondary" disabled={busy} onClick={() => void requestClear(selectedItems)}><Eraser size={14} />{t(`Clear ${selectedItems.length}`, `清空 ${selectedItems.length} 项`)}</Button>
      <Button variant="ghost-danger" disabled={busy} onClick={() => void requestDeleteMany(selectedItems)}><Trash2 size={14} />{t(`Delete ${selectedItems.length}`, `删除 ${selectedItems.length} 项`)}</Button>
    </>}
    <Button variant="secondary" onClick={() => void sessions.refetch()} disabled={busy || sessions.isFetching}><RefreshCw size={14} />{t("Refresh", "刷新")}</Button>
  </>}>
    <FieldGrid columns={3}>
      <SettingsField label={t("Search", "搜索")} anchor="session-data.search"><SkTextInput type="search" value={query} onChange={setQuery} placeholder={t("Workspace, session or date", "工作区、会话或日期")} /></SettingsField>
      <SettingsField label={t("Workspace", "工作区")} anchor="session-data.workspace"><SkSelect value={workspaceId} onChange={setWorkspaceId} options={[{ value: "", label: t("All workspaces", "全部工作区") }, ...workspaces]} /></SettingsField>
      <SettingsField label={t("Minimum size", "最小大小")} anchor="session-data.size"><SkSelect value={minimum} onChange={setMinimum} options={[{ value: "0", label: t("Any size", "不限") }, ...[1, 10, 100].map((size) => ({ value: String(size * 1024 * 1024), label: `${size} MB` }))]} /></SettingsField>
    </FieldGrid>
    {sessions.isLoading && <p className="text-xs">{t("Loading session data", "正在读取会话数据")}</p>}
    {error && <div className="settings-inline-error">{error.message}</div>}
    {deleteWarning && <p className="text-xs">{deleteWarning}</p>}
    <SessionDataTable rows={visible} selected={selectedKeys} busy={busy} onSelect={setSelectedKeys} onDetails={(session) => setExpandedId(sessionKey(session))} onClear={(session) => void requestClear([session])} onDelete={(session) => void requestDelete(session)} />
    <Modal open={Boolean(detailed)} title={detailed?.title ?? t("Session details", "会话详情")} onClose={() => setExpandedId(null)}>{detailed && <SessionDataDetails session={detailed} />}</Modal>
  </SettingsPanel>;
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
