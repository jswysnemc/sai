import { Minus, Plus, RotateCcw, Trash2, X } from "../../../shared/ui/icons";
import type { GitStatusEntry } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";

type ChangeSelectionBarProps = {
  entries: GitStatusEntry[];
  busy: boolean;
  onStage: (paths: string[]) => void;
  onUnstage: (paths: string[]) => void;
  onDiscard: (paths: string[]) => void;
  onClear: () => void;
};

/**
 * 多选生效时给出批量操作。单击仍只打开差异，Ctrl/Cmd 点选、Shift 连选。
 *
 * @param props 当前选中文件与批量操作回调
 * @returns 选择操作条，不足两项时不渲染
 */
export function ChangeSelectionBar(props: ChangeSelectionBarProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  if (props.entries.length < 2) return null;

  const stagePaths = props.entries
    .filter((entry) => entry.untracked || entry.conflicted || entry.worktree_status !== ".")
    .map((entry) => entry.path);
  const unstagePaths = props.entries.filter((entry) => entry.staged).map((entry) => entry.path);
  const discardPaths = props.entries
    .filter((entry) => entry.untracked || entry.worktree_status !== ".")
    .map((entry) => entry.path);
  const includesUntracked = props.entries.some((entry) => entry.untracked);

  /** 确认后丢弃所选工作区改动。 */
  const discard = async () => {
    const accepted = await confirm({
      title: t(`Discard ${discardPaths.length} selected changes?`, `丢弃选中的 ${discardPaths.length} 项改动？`),
      description: t("Tracked files will be restored and untracked files will be permanently deleted.", "已跟踪文件将恢复，未跟踪文件将永久删除。"),
      confirmLabel: t("Discard", "丢弃"),
      danger: true
    });
    if (accepted) props.onDiscard(discardPaths);
  };

  return (
    <div className="git-selection-bar" role="toolbar" aria-label={t("Selected changes", "已选变更")}>
      <span>{t(`${props.entries.length} selected`, `已选 ${props.entries.length}`)}</span>
      {stagePaths.length > 0 && (
        <Button disabled={props.busy} onClick={() => props.onStage(stagePaths)}>
          <Plus size={12} />{t("Stage", "暂存")}
        </Button>
      )}
      {unstagePaths.length > 0 && (
        <Button disabled={props.busy} onClick={() => props.onUnstage(unstagePaths)}>
          <Minus size={12} />{t("Unstage", "取消暂存")}
        </Button>
      )}
      {discardPaths.length > 0 && (
        <Button variant="ghost-danger" disabled={props.busy} onClick={() => void discard()}>
          {includesUntracked ? <Trash2 size={12} /> : <RotateCcw size={12} />}
          {t("Discard", "丢弃")}
        </Button>
      )}
      <Button className="git-selection-clear" onClick={props.onClear} aria-label={t("Clear selection", "清除选择")}>
        <X size={12} />
      </Button>
    </div>
  );
}
