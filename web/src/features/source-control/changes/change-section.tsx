import { ChevronRight, Minus, Plus } from "../../../shared/ui/icons";
import { useEffect, useRef, useState } from "react";
import type { GitStatusEntry, ScmConfig } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { ChangeFileList } from "./change-file-list";

export type ChangeSectionKind = "merge" | "staged" | "changes" | "untracked";

type ChangeSectionProps = {
  title: string;
  entries: GitStatusEntry[];
  selectedPath: string | null;
  selectedPaths: ReadonlySet<string>;
  viewMode: ScmConfig["default_view_mode"];
  busy: boolean;
  section: ChangeSectionKind;
  onSelect: (path: string, event: React.MouseEvent<HTMLButtonElement>) => void;
  onToggle: (path: string) => void;
  onSelectAll: (selected: boolean) => void;
  onContextMenu: (path: string, event: React.MouseEvent<HTMLDivElement>) => void;
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
  const checkRef = useRef<HTMLInputElement>(null);
  const canStageAll = props.section === "changes" || props.section === "untracked" || props.section === "merge";
  const selectedCount = props.entries.filter((entry) => props.selectedPaths.has(entry.path)).length;
  const allSelected = props.entries.length > 0 && selectedCount === props.entries.length;
  useEffect(() => {
    if (checkRef.current) checkRef.current.indeterminate = selectedCount > 0 && !allSelected;
  }, [allSelected, selectedCount]);
  return (
    <div className={`git-section git-section-${props.section}`}>
      <div className="git-change-head">
        <Button className="git-section-toggle" onClick={() => setOpen((value) => !value)} aria-expanded={open}>
          <ChevronRight size={12} className={open ? "open" : ""} />
          <span>{props.title}</span>
        </Button>
        <input
          ref={checkRef}
          type="checkbox"
          className="git-section-check"
          checked={allSelected}
          disabled={props.entries.length === 0}
          aria-label={t("Select all files in this section", "选择本分区全部文件")}
          onChange={() => props.onSelectAll(!allSelected)}
        />
        <span className="git-section-count tabular-nums">{props.entries.length}</span>
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
        <ChangeFileList
          entries={props.entries}
          viewMode={props.viewMode}
          selectedPath={props.selectedPath}
          selectedPaths={props.selectedPaths}
          busy={props.busy}
          section={props.section}
          onSelect={props.onSelect}
          onToggle={props.onToggle}
          onContextMenu={props.onContextMenu}
          onStage={props.onStage}
          onUnstage={props.onUnstage}
          onIgnore={props.onIgnore}
          onDiscard={props.onDiscard}
        />
      )}
    </div>
  );
}
