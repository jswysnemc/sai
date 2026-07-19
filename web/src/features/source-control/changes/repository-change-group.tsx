import { ChevronDown, FolderGit2, GitBranch } from "lucide-react";
import { useMemo, useState } from "react";
import type { GitRepositoryState, GitStatusEntry } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import type { GitOperationUiOptions, RunGitOperation } from "../types";
import { ChangeSection, type ChangeSectionKind } from "./change-section";
import { groupGitChanges } from "./change-groups";
import "./repository-change-group.css";

type RepositoryChangeGroupProps = {
  name: string;
  state: GitRepositoryState;
  active: boolean;
  selectedPath: string | null;
  busy: boolean;
  runOperation: RunGitOperation;
  onSelectRepository: () => void;
  onSelectChange: (path: string, section: ChangeSectionKind) => void;
};

/**
 * 渲染单个仓库的全部 Source Control 文件分区。
 *
 * @param props 仓库状态、选择状态和 Git 操作回调
 * @returns 可折叠仓库变更分区
 */
export function RepositoryChangeGroup(props: RepositoryChangeGroupProps) {
  const { t } = useI18n();
  const [open, setOpen] = useState(true);
  const groups = useMemo(() => groupGitChanges(props.state.entries), [props.state.entries]);
  const changed = props.state.entries.length;

  /**
   * 对当前仓库执行 Git 操作。
   *
   * @param action Git 操作名称
   * @param options 可选操作参数
   * @returns Git 操作结果
   */
  const run = (action: string, options: GitOperationUiOptions = {}) => props.runOperation(action, {
    ...options,
    repo_root: props.state.repo_root
  });

  /**
   * 变更文件暂存状态，并在操作成功后更新所选 Diff 分区。
   *
   * @param action 暂存或取消暂存操作
   * @param path 仓库相对文件路径
   * @param nextSection 操作成功后的目标分区
   * @returns 无返回值
   */
  const move = async (
    action: "stage" | "unstage",
    path: string,
    nextSection: ChangeSectionKind
  ) => {
    const result = await run(action, { path });
    if (result?.ok) props.onSelectChange(path, nextSection);
  };

  /**
   * 经确认后丢弃当前仓库的单个文件变化。
   *
   * @param entry 待丢弃文件状态
   * @returns 无返回值
   */
  const discard = (entry: GitStatusEntry) => {
    void run("discard", {
      path: entry.path,
      old_path: entry.old_path ?? undefined,
      confirmTitle: entry.untracked
        ? t("Delete untracked file", "删除未跟踪文件")
        : t("Discard working tree changes", "撤销工作区修改"),
      confirmDescription: entry.untracked
        ? t(`Permanently delete “${entry.path}”.`, `将永久删除“${entry.path}”。`)
        : t(`Restore ${entry.path}. Unsaved changes cannot be recovered.`, `将恢复 ${entry.path}，未保存修改无法恢复。`)
    });
  };

  const selectedPath = props.active ? props.selectedPath : null;
  return (
    <section className={`git-repository-changes${props.active ? " active" : ""}`}>
      <header className="git-repository-changes-head">
        <Button className="git-repository-changes-toggle" onClick={() => setOpen((value) => !value)}>
          <ChevronDown size={12} className={open ? "open" : ""} />
          <FolderGit2 size={13} />
          <span>
            <strong>{props.name}</strong>
            <small><GitBranch size={10} />{props.state.head || "HEAD"}</small>
          </span>
          <em>{changed}</em>
        </Button>
        {!props.active && (
          <Button onClick={props.onSelectRepository}>{t("Select", "选择")}</Button>
        )}
      </header>
      {open && (
        <div className="git-repository-change-sections">
          {groups.conflicts.length > 0 && (
            <ChangeSection
              title={t(`Merge Changes ${groups.conflicts.length}`, `合并变更 ${groups.conflicts.length}`)}
              entries={groups.conflicts}
              selectedPath={selectedPath}
              busy={props.busy}
              onSelect={(path) => props.onSelectChange(path, "merge")}
              onStageAll={() => void run("stage_all")}
              onUnstageAll={() => void run("unstage_all")}
              onStage={(path) => void move("stage", path, "staged")}
              onUnstage={(path) => void move("unstage", path, "changes")}
              onIgnore={(path) => void run("add_to_gitignore", { path })}
              onDiscard={discard}
              section="merge"
            />
          )}
          {groups.staged.length > 0 && (
            <ChangeSection
              title={t(`Staged Changes ${groups.staged.length}`, `已暂存变更 ${groups.staged.length}`)}
              entries={groups.staged}
              selectedPath={selectedPath}
              busy={props.busy}
              onSelect={(path) => props.onSelectChange(path, "staged")}
              onStageAll={() => void run("stage_all")}
              onUnstageAll={() => void run("unstage_all")}
              onStage={(path) => void move("stage", path, "staged")}
              onUnstage={(path) => void move("unstage", path, "changes")}
              onIgnore={(path) => void run("add_to_gitignore", { path })}
              onDiscard={discard}
              section="staged"
            />
          )}
          {groups.changes.length > 0 && (
            <ChangeSection
              title={t(`Changes ${groups.changes.length}`, `更改 ${groups.changes.length}`)}
              entries={groups.changes}
              selectedPath={selectedPath}
              busy={props.busy}
              onSelect={(path) => props.onSelectChange(path, "changes")}
              onStageAll={() => void run("stage_all")}
              onUnstageAll={() => void run("unstage_all")}
              onStage={(path) => void move("stage", path, "staged")}
              onUnstage={(path) => void move("unstage", path, "changes")}
              onIgnore={(path) => void run("add_to_gitignore", { path })}
              onDiscard={discard}
              section="changes"
            />
          )}
          {groups.untracked.length > 0 && (
            <ChangeSection
              title={t(`Untracked ${groups.untracked.length}`, `未跟踪 ${groups.untracked.length}`)}
              entries={groups.untracked}
              selectedPath={selectedPath}
              busy={props.busy}
              onSelect={(path) => props.onSelectChange(path, "untracked")}
              onStageAll={() => void run("stage_all")}
              onUnstageAll={() => void run("unstage_all")}
              onStage={(path) => void move("stage", path, "staged")}
              onUnstage={(path) => void move("unstage", path, "changes")}
              onIgnore={(path) => void run("add_to_gitignore", { path })}
              onDiscard={discard}
              section="untracked"
            />
          )}
          {changed === 0 && <div className="git-clean">{t("No changes", "没有变更")}</div>}
        </div>
      )}
    </section>
  );
}
