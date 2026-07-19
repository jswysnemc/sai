import { ChevronDown, EyeOff, Minus, Plus, RotateCcw, Trash2 } from "lucide-react";
import { useState } from "react";
import type { GitStatusEntry } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { FileTypeIcon } from "../../../shared/ui/file-icon";
import { useI18n } from "../../i18n/use-i18n";

export type ChangeSectionKind = "merge" | "staged" | "changes" | "untracked";

type ChangeSectionProps = {
  title: string;
  entries: GitStatusEntry[];
  selectedPath: string | null;
  busy: boolean;
  section: ChangeSectionKind;
  onSelect: (path: string) => void;
  onStageAll: () => void;
  onUnstageAll: () => void;
  onStage: (path: string) => void;
  onUnstage: (path: string) => void;
  onIgnore: (path: string) => void;
  onDiscard: (entry: GitStatusEntry) => void;
};

/**
 * 渲染一个 Source Control 文件分区及其行内操作。
 *
 * @param props 分区类型、文件状态和操作回调
 * @returns 可折叠文件分区
 */
export function ChangeSection(props: ChangeSectionProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(true);
  const canStageAll = props.section === "changes" || props.section === "untracked" || props.section === "merge";
  return (
    <div className={`git-section git-section-${props.section}`}>
      <div className="git-change-head">
        <Button className="git-section-toggle" onClick={() => setOpen((value) => !value)}>
          <ChevronDown size={12} className={open ? "open" : ""} />
          <span>{props.title}</span>
        </Button>
        <span>
          {props.section === "staged" ? (
            <Button className="git-icon-action" onClick={props.onUnstageAll} title={t("Unstage all", "取消全部暂存")} disabled={props.busy}>
              <Minus size={12} />
            </Button>
          ) : canStageAll && props.entries.length > 0 ? (
            <Button className="git-icon-action" onClick={props.onStageAll} title={t("Stage all", "暂存全部")} disabled={props.busy}>
              <Plus size={12} />
            </Button>
          ) : null}
        </span>
      </div>
      {open && (
        <div className="git-file-list">
          {props.entries.map((entry) => (
            <div
              className={`git-file-row${props.selectedPath === entry.path ? " active" : ""}`}
              key={`${props.section}:${entry.index_status}${entry.worktree_status}:${entry.path}`}
            >
              <Button className="git-file-main" onClick={() => props.onSelect(entry.path)}>
                <FileTypeIcon name={entry.path} size={13} />
                <span className="git-file-path" title={entry.path}>
                  <strong>{entry.path}</strong>
                  {entry.old_path && <small>{entry.old_path} → {entry.path}</small>}
                </span>
                <span className={`git-file-status tone-${statusTone(entry)}`}>{statusLabel(entry)}</span>
              </Button>
              <span className="git-file-actions">
                {props.section === "staged" && (
                  <Button className="git-icon-action" disabled={props.busy} onClick={() => props.onUnstage(entry.path)} title={t("Unstage", "取消暂存")}>
                    <Minus size={12} />
                  </Button>
                )}
                {(props.section === "changes" || props.section === "untracked" || props.section === "merge") && (
                  <Button className="git-icon-action" disabled={props.busy} onClick={() => props.onStage(entry.path)} title={props.section === "merge" ? t("Mark as resolved", "标记为已解决") : t("Stage", "暂存")}>
                    <Plus size={12} />
                  </Button>
                )}
                {props.section === "untracked" && (
                  <Button className="git-icon-action" disabled={props.busy} onClick={() => props.onIgnore(entry.path)} title={t("Add to .gitignore", "加入 .gitignore")}>
                    <EyeOff size={12} />
                  </Button>
                )}
                {(props.section === "changes" || props.section === "untracked") && (
                  <Button
                    className="git-icon-action"
                    disabled={props.busy}
                    onClick={() => props.onDiscard(entry)}
                    title={entry.untracked ? t("Delete untracked file", "删除未跟踪文件") : t("Discard changes", "撤销修改")}
                  >
                    {entry.untracked ? <Trash2 size={12} /> : <RotateCcw size={12} />}
                  </Button>
                )}
              </span>
            </div>
          ))}
          {props.entries.length === 0 && <div className="git-clean">{t("No files", "无文件")}</div>}
        </div>
      )}
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
