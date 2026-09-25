import { useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "../../api/client";
import { ContextActionMenu } from "../../shared/ui/menu/context-action-menu";
import { stripExtendedPathPrefix } from "../workspace/workspace-path-utils";
import { matchesStoredSessionId, nextPinnedIds, sidebarSessionKey } from "./sidebar-session-key";
import { useI18n } from "../i18n/use-i18n";

type SidebarSessionContextMenuProps = {
  sessionId: string;
  workspaceId?: string;
  sameIdCount?: number;
  title: string;
  workspacePath: string;
  x: number;
  y: number;
  archived: boolean;
  onClose: () => void;
  onRename: () => void;
  onDelete: () => void;
  /** 当前会话属于活动工作区时提供多选入口 */
  onEnterSelection?: () => void;
};

/**
 * 渲染会话右键菜单，并写回置顶和未读。
 *
 * @param props 会话标识、坐标和重命名、删除回调
 * @returns 右键菜单
 */
export function SidebarSessionContextMenu({ sessionId, workspaceId = "", sameIdCount = 1, workspacePath, x, y, onClose, onRename, onDelete, onEnterSelection }: SidebarSessionContextMenuProps) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const index = useQuery({ queryKey: ["session-sidebar"], queryFn: () => api.sessionSidebar.read() });
  const indexKey = workspaceId ? sidebarSessionKey(workspaceId, sessionId) : sessionId;
  const pinned = (index.data?.pinned ?? []).some((id) =>
    workspaceId ? matchesStoredSessionId(id, workspaceId, sessionId, sameIdCount) : id === sessionId
  );

  /**
   * 写回侧栏索引并刷新列表。
   *
   * @param patch 局部更新
   */
  const save = (patch: Parameters<typeof api.sessionSidebar.update>[0]) => {
    void api.sessionSidebar.update(patch).then(() => queryClient.invalidateQueries({ queryKey: ["session-sidebar"] }));
  };

  return (
    <ContextActionMenu
      label={t("Task actions", "任务操作")}
      x={x}
      y={y}
      onClose={onClose}
      items={[
        {
          id: "pin",
          label: pinned ? t("Unpin", "取消置顶") : t("Pin", "置顶"),
          onSelect: () => {
            const current = index.data?.pinned ?? [];
            save({
              pinned: workspaceId
                ? nextPinnedIds(current, workspaceId, sessionId, pinned)
                : pinned
                  ? current.filter((id) => id !== sessionId)
                  : [sessionId, ...current]
            });
          }
        },
        { id: "rename", label: t("Rename", "重命名"), onSelect: onRename },
        ...(onEnterSelection ? [{ id: "select", label: t("Select sessions", "多选会话"), onSelect: onEnterSelection }] : []),
        { id: "unread", label: t("Mark as unread", "标记为未读"), onSelect: () => save({ mark_unread: indexKey }) },
        { id: "delete", label: t("Delete", "删除"), danger: true, separator: true, onSelect: onDelete },
        { id: "copy-id", label: t("Copy session id", "复制会话标识"), separator: true, onSelect: () => void navigator.clipboard.writeText(sessionId) },
        { id: "copy-path", label: t("Copy workspace path", "复制工作区路径"), onSelect: () => void navigator.clipboard.writeText(stripExtendedPathPrefix(workspacePath)) }
      ]}
    />
  );
}
