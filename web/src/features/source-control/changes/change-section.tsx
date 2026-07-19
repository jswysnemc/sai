import { ChevronDown, Minus, Plus } from "lucide-react";
import { useState } from "react";
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
        <ChangeFileList
          entries={props.entries}
          viewMode={props.viewMode}
          selectedPath={props.selectedPath}
          selectedPaths={props.selectedPaths}
          busy={props.busy}
          section={props.section}
          onSelect={props.onSelect}
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
