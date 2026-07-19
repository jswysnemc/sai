import { EyeOff, Minus, Plus, RotateCcw, Trash2 } from "lucide-react";
import type { CSSProperties } from "react";
import type { GitStatusEntry } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { FileTypeIcon } from "../../../shared/ui/file-icon";
import { useI18n } from "../../i18n/use-i18n";
import type { ChangeSectionKind } from "./change-section";

type ChangeFileRowProps = {
  entry: GitStatusEntry;
  displayName: string;
  depth: number;
  active: boolean;
  selected: boolean;
  busy: boolean;
  section: ChangeSectionKind;
  onSelect: (event: React.MouseEvent<HTMLButtonElement>) => void;
  onContextMenu: (event: React.MouseEvent<HTMLDivElement>) => void;
  onStage: () => void;
  onUnstage: () => void;
  onIgnore: () => void;
  onDiscard: () => void;
};

/**
 * 渲染单个 Git 文件及其行内操作。
 *
 * @param props 文件状态、树形深度和操作回调
 * @returns 文件行
 */
export function ChangeFileRow(props: ChangeFileRowProps) {
  const { t } = useI18n();
  const style = { "--git-tree-indent": `${props.depth * 0.875}rem` } as CSSProperties;
  const canStage = props.section === "changes" || props.section === "untracked" || props.section === "merge";
  const canDiscard = props.section === "changes" || props.section === "untracked";

  return (
    <div
      className={`git-file-row${props.active ? " active" : ""}${props.selected ? " selected" : ""}`}
      style={style}
      onContextMenu={props.onContextMenu}
    >
      <Button className="git-file-main" onClick={props.onSelect} title={props.entry.path}>
        <FileTypeIcon name={props.entry.path} size={13} />
        <span className="git-file-path">
          <strong>{props.displayName}</strong>
          {props.entry.old_path && <small>{props.entry.old_path} → {props.entry.path}</small>}
        </span>
        <span className={`git-file-status tone-${statusTone(props.entry)}`}>{statusLabel(props.entry)}</span>
      </Button>
      <span className="git-file-actions">
        {props.section === "staged" && (
          <Button className="git-icon-action" disabled={props.busy} onClick={props.onUnstage} title={t("Unstage", "取消暂存")}>
            <Minus size={12} />
          </Button>
        )}
        {canStage && (
          <Button className="git-icon-action" disabled={props.busy} onClick={props.onStage} title={props.section === "merge" ? t("Mark as resolved", "标记为已解决") : t("Stage", "暂存")}>
            <Plus size={12} />
          </Button>
        )}
        {props.entry.untracked && (
          <Button className="git-icon-action" disabled={props.busy} onClick={props.onIgnore} title={t("Add to .gitignore", "加入 .gitignore")}>
            <EyeOff size={12} />
          </Button>
        )}
        {canDiscard && (
          <Button
            className="git-icon-action"
            disabled={props.busy}
            onClick={props.onDiscard}
            title={props.entry.untracked ? t("Delete untracked file", "删除未跟踪文件") : t("Discard changes", "撤销修改")}
          >
            {props.entry.untracked ? <Trash2 size={12} /> : <RotateCcw size={12} />}
          </Button>
        )}
      </span>
    </div>
  );
}

/**
 * 返回文件状态短标签。
 *
 * @param entry Git 文件状态
 * @returns 单字符或组合状态标签
 */
function statusLabel(entry: GitStatusEntry): string {
  if (entry.conflicted) return "U";
  if (entry.untracked) return "U";
  if (entry.staged && entry.worktree_status !== ".") return "M*";
  if (entry.staged) return entry.index_status === "A" ? "A" : entry.index_status === "D" ? "D" : "M";
  if (entry.worktree_status === "D") return "D";
  return "M";
}

/**
 * 返回文件状态对应的视觉色调。
 *
 * @param entry Git 文件状态
 * @returns CSS 色调名称
 */
function statusTone(entry: GitStatusEntry): string {
  if (entry.conflicted) return "conflict";
  if (entry.untracked) return "untracked";
  if (entry.worktree_status === "D" || entry.index_status === "D") return "deleted";
  if (entry.index_status === "A") return "added";
  return "modified";
}
