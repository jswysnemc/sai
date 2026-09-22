import { useQuery, useQueryClient } from "@tanstack/react-query";
import { CheckSquare2, ListTree, Pin, PinOff, Square } from "lucide-react";
import type { MouseEvent } from "react";
import { api } from "../../api/client";
import { Button } from "../../shared/ui/button/button";
import { formatRelativeTime } from "../../shared/format-relative-time";
import type { Session } from "../../api/contracts";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { OPEN_SIDEBAR_FILE_TREE_EVENT } from "./sidebar-file-tree-cover";
import { useI18n } from "../i18n/use-i18n";
import "./session-row.css";

type SessionRowProps = {
  session: Session;
  loaded: boolean;
  running: boolean;
  holder?: string | null;
  unread?: boolean;
  /** 相对时间的基准时刻，由列表统一按分钟推进 */
  now: number;
  /** 多选模式下是否可勾选（仅活动工作区） */
  selectable: boolean;
  checked: boolean;
  /** 行菜单里是否提供多选入口（仅活动工作区） */
  canSelect: boolean;
  /** 活动工作区才显示悬停动作 */
  canManage?: boolean;
  onOpen: () => void;
  onToggleChecked: () => void;
  onStartRename: () => void;
  onEnterSelection: () => void;
  onDelete: () => void;
  onContextMenu?: (event: MouseEvent) => void;
};

/**
 * 渲染单条会话行：标题、相对时间、加载指示与行内操作。
 *
 * 悬停时右侧换成文件树、置顶和归档，时间让位。
 *
 * @param props 会话数据、行状态与操作回调
 * @returns 会话行
 */
export function SessionRow({
  session,
  loaded,
  running,
  holder,
  unread = false,
  now,
  selectable,
  checked,
  canManage = true,
  onOpen,
  onToggleChecked,
  onContextMenu
}: SessionRowProps) {
  const { locale, t } = useI18n();
  const queryClient = useQueryClient();
  const index = useQuery({ queryKey: ["session-sidebar"], queryFn: () => api.sessionSidebar.read(), enabled: canManage });
  const pinned = index.data?.pinned.includes(session.id) ?? false;

  /**
   * 写回置顶或归档，并刷新侧栏索引。
   *
   * @param patch 变更字段
   */
  const saveIndex = (patch: Parameters<typeof api.sessionSidebar.update>[0]) => {
    void api.sessionSidebar.update(patch).then(() => queryClient.invalidateQueries({ queryKey: ["session-sidebar"] }));
  };
  const rowClass = [
    "session-row",
    session.active ? "active" : "",
    checked ? "selected" : "",
    loaded ? "loaded" : "unloaded"
  ].filter(Boolean).join(" ");

  return (
    <div className={rowClass} onContextMenu={onContextMenu}>
      {selectable && (
        <Button variant="ghost" className="session-check" onClick={onToggleChecked} aria-label={t(`Select ${session.title}`, `选择 ${session.title}`)}>
          {checked ? <CheckSquare2 size={15} /> : <Square size={15} />}
        </Button>
      )}
        <Button variant="ghost" className="session-main" aria-current={session.active ? "page" : undefined} onClick={selectable ? onToggleChecked : onOpen}>
          <span className="session-summary">
            <strong>{session.title}</strong>
            {unread && <i className="session-unread" aria-label={t("Unread", "未读")} />}
            {running && <ActiveAgentIndicator holder={holder} />}
            <small title={new Date(session.updated_at).toLocaleString(locale)}>
              {formatRelativeTime(session.updated_at, locale, now)}
            </small>
          </span>
        </Button>
      {!selectable && canManage && (
        <div className="session-hover-actions">
          <Button
            variant="ghost"
            size="icon"
            title={t("Show file tree", "显示文件树")}
            aria-label={t("Show file tree", "显示文件树")}
            onClick={(event) => {
              event.stopPropagation();
              window.dispatchEvent(new Event(OPEN_SIDEBAR_FILE_TREE_EVENT));
            }}
          >
            <ListTree size={14} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            title={pinned ? t("Unpin", "取消置顶") : t("Pin", "置顶")}
            aria-label={pinned ? t("Unpin", "取消置顶") : t("Pin", "置顶")}
            onClick={(event) => {
              event.stopPropagation();
              const current = index.data?.pinned ?? [];
              saveIndex({ pinned: pinned ? current.filter((id) => id !== session.id) : [session.id, ...current] });
            }}
          >
            {pinned ? <PinOff size={14} /> : <Pin size={14} />}
          </Button>
        </div>
      )}
    </div>
  );
}
