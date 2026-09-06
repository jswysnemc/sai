import { CheckSquare2, MoreHorizontal, Pencil, Square, Trash2 } from "lucide-react";
import { Button } from "../../shared/ui/button/button";
import { ActionMenu } from "../../shared/ui/menu/action-menu";
import { formatRelativeTime } from "../../shared/format-relative-time";
import type { Session } from "../../api/contracts";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { useI18n } from "../i18n/use-i18n";
import "./session-row.css";

type SessionRowProps = {
  session: Session;
  loaded: boolean;
  running: boolean;
  holder?: string | null;
  /** 相对时间的基准时刻，由列表统一按分钟推进 */
  now: number;
  /** 多选模式下是否可勾选（仅活动工作区） */
  selectable: boolean;
  checked: boolean;
  /** 行菜单里是否提供多选入口（仅活动工作区） */
  canSelect: boolean;
  canManage?: boolean;
  onOpen: () => void;
  onToggleChecked: () => void;
  onStartRename: () => void;
  onEnterSelection: () => void;
  onDelete: () => void;
};

/**
 * 渲染单条会话行：标题、相对时间、加载指示与行内操作。
 *
 * 悬停时更多按钮从右缘滑入、时间整体左移让位，两者始终共存——
 * 时间是扫读时的主要线索，不能在悬停瞬间消失。
 *
 * @param props 会话数据、行状态与操作回调
 * @returns 会话行
 */
export function SessionRow({
  session,
  loaded,
  running,
  holder,
  now,
  selectable,
  checked,
  canSelect,
  canManage = true,
  onOpen,
  onToggleChecked,
  onStartRename,
  onEnterSelection,
  onDelete
}: SessionRowProps) {
  const { locale, t } = useI18n();
  const rowClass = [
    "session-row",
    session.active ? "active" : "",
    checked ? "selected" : "",
    loaded ? "loaded" : "unloaded"
  ].filter(Boolean).join(" ");

  return (
    <div className={rowClass}>
      {selectable && (
        <Button variant="ghost" className="session-check" onClick={onToggleChecked} aria-label={t(`Select ${session.title}`, `选择 ${session.title}`)}>
          {checked ? <CheckSquare2 size={15} /> : <Square size={15} />}
        </Button>
      )}
        <Button variant="ghost" className="session-main" aria-current={session.active ? "page" : undefined} onClick={selectable ? onToggleChecked : onOpen}>
          <span className="session-summary">
            <strong>{session.title}</strong>
            {running && <ActiveAgentIndicator holder={holder} />}
            <small title={new Date(session.updated_at).toLocaleString(locale)}>
              {formatRelativeTime(session.updated_at, locale, now)}
            </small>
          </span>
        </Button>
      {!selectable && canManage && <ActionMenu className="session-more-menu" label={t(`Manage ${session.title}`, `管理 ${session.title}`)} trigger={<MoreHorizontal size={15} />} items={[
        { id: "rename", label: t("Rename", "重命名"), icon: <Pencil size={14} />, onSelect: onStartRename },
        ...(canSelect ? [{ id: "select", label: t("Select sessions", "多选会话"), icon: <CheckSquare2 size={14} />, onSelect: onEnterSelection }] : []),
        { id: "delete", label: t("Delete", "删除"), icon: <Trash2 size={14} />, danger: true, separator: true, onSelect: onDelete }
      ]} />}
    </div>
  );
}
