import { CheckSquare2, MoreHorizontal, Pencil, Square, Trash2 } from "lucide-react";
import type { RefObject } from "react";
import { useRef } from "react";
import { formatRelativeTime } from "../../shared/format-relative-time";
import type { Session } from "../../api/contracts";
import { ActiveAgentIndicator } from "./active-agent-indicator";
import { useI18n } from "../i18n/use-i18n";
import "./session-row.css";

type SessionRowProps = {
  session: Session;
  loaded: boolean;
  holder?: string | null;
  /** 相对时间的基准时刻，由列表统一按分钟推进 */
  now: number;
  /** 多选模式下是否可勾选（仅活动工作区） */
  selectable: boolean;
  checked: boolean;
  /** 是否处于重命名编辑态 */
  renaming: boolean;
  renameDraft: string;
  renamePending: boolean;
  /** 是否显示行菜单 */
  menuOpen: boolean;
  /** 菜单外点关闭用的容器引用 */
  menuRef: RefObject<HTMLDivElement | null>;
  /** 行菜单里是否提供多选入口（仅活动工作区） */
  canSelect: boolean;
  onOpen: () => void;
  onToggleChecked: () => void;
  onToggleMenu: () => void;
  onStartRename: () => void;
  onRenameDraft: (value: string) => void;
  onRenameSubmit: () => void;
  onRenameCancel: () => void;
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
  holder,
  now,
  selectable,
  checked,
  renaming,
  renameDraft,
  renamePending,
  menuOpen,
  menuRef,
  canSelect,
  onOpen,
  onToggleChecked,
  onToggleMenu,
  onStartRename,
  onRenameDraft,
  onRenameSubmit,
  onRenameCancel,
  onEnterSelection,
  onDelete
}: SessionRowProps) {
  const { locale, t } = useI18n();
  const skipBlurSubmit = useRef(false);
  const rowClass = [
    "session-row",
    session.active ? "active" : "",
    checked ? "selected" : "",
    loaded ? "loaded" : "unloaded"
  ].filter(Boolean).join(" ");

  return (
    <div className={rowClass}>
      {selectable && (
        <button type="button" className="session-check" onClick={onToggleChecked} aria-label={t(`Select ${session.title}`, `选择 ${session.title}`)}>
          {checked ? <CheckSquare2 size={15} /> : <Square size={15} />}
        </button>
      )}
      {!selectable && renaming ? (
        <div className="session-rename">
          <input
            autoFocus
            value={renameDraft}
            disabled={renamePending}
            onChange={(event) => onRenameDraft(event.target.value)}
            onKeyDown={(event) => {
              // 1. 回车提交重命名，并跳过失焦重复提交
              if (event.key === "Enter") {
                skipBlurSubmit.current = true;
                onRenameSubmit();
              }
              // 2. Esc 取消编辑，并跳过失焦提交
              if (event.key === "Escape") {
                skipBlurSubmit.current = true;
                onRenameCancel();
              }
            }}
            onBlur={() => {
              if (skipBlurSubmit.current) {
                skipBlurSubmit.current = false;
                return;
              }
              onRenameSubmit();
            }}
            aria-label={t(`Rename ${session.title}`, `重命名 ${session.title}`)}
          />
        </div>
      ) : (
        <button type="button" className="session-main" onClick={selectable ? onToggleChecked : onOpen}>
          <span className="session-summary">
            <strong>{session.title}</strong>
            {loaded && <ActiveAgentIndicator holder={holder} />}
            <small title={new Date(session.updated_at).toLocaleString(locale)}>
              {formatRelativeTime(session.updated_at, locale, now)}
            </small>
          </span>
        </button>
      )}
      {!selectable && !renaming && (
        <button type="button" className="session-more" aria-label={t(`Manage ${session.title}`, `管理 ${session.title}`)} onClick={onToggleMenu}>
          <MoreHorizontal size={15} />
        </button>
      )}
      {!selectable && menuOpen && (
        <div className="session-menu" ref={menuRef}>
          <button type="button" onClick={onStartRename}><Pencil size={14} /> {t("Rename", "重命名")}</button>
          {canSelect && (
            <button type="button" onClick={onEnterSelection}><CheckSquare2 size={14} /> {t("Select sessions", "多选会话")}</button>
          )}
          <button type="button" className="danger" onClick={onDelete}><Trash2 size={14} /> {t("Delete", "删除")}</button>
        </div>
      )}
    </div>
  );
}
