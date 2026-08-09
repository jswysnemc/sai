import { CheckSquare2, Trash2 } from "lucide-react";
import { useI18n } from "../i18n/use-i18n";

type SessionSelectionBarProps = {
  /** 当前可选会话 ID 集合 */
  sessionIds: string[];
  selectedCount: number;
  confirming: boolean;
  busy: boolean;
  onToggleAll: (ids: string[]) => void;
  onDelete: () => void;
};

/**
 * 渲染多选模式的工具条：全选、计数与批量删除。
 *
 * @param props 选中状态与批量操作回调
 * @returns 多选工具条
 */
export function SessionSelectionBar({
  sessionIds,
  selectedCount,
  confirming,
  busy,
  onToggleAll,
  onDelete
}: SessionSelectionBarProps) {
  const { t } = useI18n();
  const allSelected = sessionIds.length > 0 && selectedCount === sessionIds.length;
  return (
    <div className="workspace-selection-bar" role="toolbar" aria-label={t("Session selection", "会话多选")}>
      <button
        type="button"
        className="selection-action"
        onClick={() => onToggleAll(sessionIds)}
        disabled={sessionIds.length === 0}
        aria-label={allSelected ? t("Clear selection", "取消全选") : t("Select all", "全选")}
        title={allSelected ? t("Clear selection", "取消全选") : t("Select all", "全选")}
      >
        <CheckSquare2 size={13} />
        <span>{allSelected ? t("Clear", "取消全选") : t("Select all", "全选")}</span>
      </button>
      <span className="workspace-selection-count">{t(`${selectedCount} selected`, `已选 ${selectedCount}`)}</span>
      <button
        type="button"
        className={confirming || busy ? "selection-delete confirming" : "selection-delete"}
        onClick={onDelete}
        disabled={selectedCount === 0 || busy}
        aria-label={t("Delete selected sessions", "删除所选会话")}
        title={t("Delete selected sessions", "删除所选会话")}
      >
        <Trash2 size={13} />
        <span>{busy ? t("Deleting", "删除中") : t("Delete", "删除")}</span>
      </button>
    </div>
  );
}
