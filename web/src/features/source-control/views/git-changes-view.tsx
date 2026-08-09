import { memo, useCallback } from "react";
import type { GitConfig, GitRepositoryState, ScmConfig } from "../../../api/contracts";
import type { GitOperationOptions } from "../../../api/git-contracts";
import { Button } from "../../../shared/ui/button/button";
import { useI18n } from "../../i18n/use-i18n";
import { CommitControl } from "../changes/commit-control";
import type { ChangeSectionKind } from "../changes/change-section";
import { RepositoryChangeGroup } from "../changes/repository-change-group";
import { MergeEditor } from "../conflicts/merge-editor";
import { FileComparisonView } from "../diff/file-comparison-view";
import { SourceControlDiff } from "../diff/source-control-diff";
import type { useFileComparison } from "../diff/use-file-comparison";
import { InProgressOperationBar } from "../operation/in-progress-operation-bar";
import { SourceControlSplitView } from "../layout/source-control-split-view";
import { PublishRepositoryControl } from "../remote/publish-repository-control";
import type { RunGitOperation } from "../types";
import "./git-views.css";

type GitChangesViewProps = {
  state: GitRepositoryState;
  allStates: GitRepositoryState[];
  repositoryNames: Map<string, string>;
  selectedRoot: string | null;
  onSelectRoot: (root: string) => void;
  scm: ScmConfig;
  git: GitConfig;
  busy: boolean;
  dirtyTotal: number;
  /** 变更选择与提交说明等会话内状态 */
  scmState: {
    message: string;
    diffMode: string;
    selectedPath: string | null;
    selectedSection: string;
    setMessage: (message: string) => void;
    setDiffMode: (mode: "changes" | "branch") => void;
    setSelectedSection: (section: ChangeSectionKind) => void;
    selectRepositoryChange: (root: string, path: string, section: ChangeSectionKind) => void;
  };
  fileComparison: ReturnType<typeof useFileComparison>;
  conflictCount: number;
  stagedCount: number;
  workingCount: number;
  remoteUrl: string;
  onRemoteUrlChange: (url: string) => void;
  reviewDiff: { data?: unknown; isLoading: boolean; error: Error | null };
  suggestingMessage: boolean;
  onSuggestMessage: () => void;
  onCommit: (options: GitOperationOptions) => Promise<boolean>;
  runOperation: RunGitOperation;
};

/**
 * 渲染变更视图：左侧仓库变更列表，右侧差异详情。
 *
 * @param props 仓库状态、选择状态与操作回调
 * @returns 变更视图
 */
export function GitChangesView(props: GitChangesViewProps) {
  const { t } = useI18n();
  const { fileComparison, scmState, state } = props;

  const handleSelectRepository = useCallback(
    (root: string) => {
      fileComparison.clear();
      props.onSelectRoot(root);
    },
    [fileComparison, props]
  );

  const handleSelectChange = useCallback(
    (root: string, path: string, section: ChangeSectionKind) => {
      fileComparison.clear();
      scmState.selectRepositoryChange(root, path, section);
      props.onSelectRoot(root);
    },
    [fileComparison, props, scmState]
  );

  return (
    <SourceControlSplitView
      className="git-changes-body"
      detailKey={fileComparison.target?.headPath ?? scmState.selectedPath}
      detailTitle={fileComparison.target?.headPath ?? scmState.selectedPath ?? undefined}
    >
      <section className="git-change-panel">
        {state.operation && (
          <InProgressOperationBar
            operation={state.operation}
            conflictedCount={props.conflictCount}
            busy={props.busy}
            runOperation={props.runOperation}
          />
        )}
        <CommitControl
          message={scmState.message}
          stagedCount={props.stagedCount}
          workingCount={props.workingCount}
          conflictedCount={props.conflictCount}
          busy={props.busy}
          enableSmartCommit={props.git.enable_smart_commit}
          suggestSmartCommit={props.git.suggest_smart_commit}
          showActionButton={props.git.show_action_button}
          confirmEmptyCommits={props.git.confirm_empty_commits}
          confirmSync={props.git.confirm_sync}
          postCommitCommand={props.git.post_commit_command}
          untrackedChanges={props.git.untracked_changes}
          onMessageChange={scmState.setMessage}
          onCommit={props.onCommit}
          allowSuggestMessage={props.git.auto_commit_message_enabled !== false}
          suggestingMessage={props.suggestingMessage}
          onSuggestMessage={props.onSuggestMessage}
        />

        <div className="git-change-scroll">
          {props.allStates.map((repository) => (
            <RepositoryChangeGroupMemo
              key={repository.repo_root}
              name={props.repositoryNames.get(repository.repo_root) ?? fallbackName(repository.repo_root)}
              state={repository}
              active={repository.repo_root === props.selectedRoot}
              selectedPath={scmState.selectedPath}
              viewMode={props.scm.default_view_mode}
              untrackedMode={props.git.untracked_changes}
              busy={props.busy}
              runOperation={props.runOperation}
              comparisonBasePath={fileComparison.bases[repository.repo_root] ?? null}
              onSelectRepository={() => handleSelectRepository(repository.repo_root)}
              onSelectChange={(path, section) => handleSelectChange(repository.repo_root, path, section)}
              onSelectForCompare={(path) => fileComparison.selectBase(repository.repo_root, path)}
              onCompareWithSelected={(path) => {
                fileComparison.compare(repository.repo_root, path);
                props.onSelectRoot(repository.repo_root);
              }}
            />
          ))}
        </div>

        <footer className="git-change-footer">
          <div className="git-diff-mode" role="group" aria-label={t("Diff baseline", "差异基准")}>
            <Button
              className={scmState.diffMode === "changes" ? "active" : ""}
              onClick={() => scmState.setDiffMode("changes")}
            >
              {t("Selected changes", "所选变更")}
            </Button>
            <Button
              className={scmState.diffMode === "branch" ? "active" : ""}
              onClick={() => scmState.setDiffMode("branch")}
            >
              {t("Against baseline", "相对基线")}
            </Button>
          </div>
          {props.dirtyTotal > 0 && (
            <Button
              className="git-discard-all"
              variant="ghost-danger"
              disabled={props.busy}
              onClick={() =>
                void props.runOperation("discard_all", {
                  confirmTitle: t("Discard all changes", "丢弃全部改动"),
                  confirmDescription: t(
                    "Discard all staged, unstaged, and untracked changes. This action cannot be undone.",
                    "将放弃所有已暂存、未暂存和未跟踪改动，此操作无法撤销。"
                  ),
                })
              }
            >
              {t("Discard all", "全部丢弃")}
            </Button>
          )}
        </footer>

        <PublishRepositoryControl
          remoteUrl={props.remoteUrl}
          remoteConfigured={Boolean(state.remote_url)}
          canPublish={!state.remote_url && state.has_commits}
          busy={props.busy}
          onRemoteUrlChange={props.onRemoteUrlChange}
          onSave={() => void props.runOperation("set_remote", { remote_url: props.remoteUrl })}
          onPublish={() => void props.runOperation("publish", { remote_url: props.remoteUrl })}
        />
      </section>

      <div className="diff-scroll">
        {fileComparison.target ? (
          <FileComparisonView
            target={fileComparison.target}
            data={fileComparison.data}
            loading={fileComparison.loading}
            error={fileComparison.error}
            busy={props.busy}
            runOperation={props.runOperation}
            onClose={fileComparison.clear}
          />
        ) : scmState.selectedSection === "merge" && scmState.selectedPath ? (
          <MergeEditor
            path={scmState.selectedPath}
            repoRoot={props.selectedRoot}
            busy={props.busy}
            runOperation={props.runOperation}
            onResolved={() => scmState.setSelectedSection("staged")}
          />
        ) : (
          <SourceControlDiff
            data={props.reviewDiff.data as never}
            loading={props.reviewDiff.isLoading}
            error={props.reviewDiff.error}
            selectedPath={scmState.selectedPath}
            busy={props.busy}
            runOperation={props.runOperation}
          />
        )}
      </div>
    </SourceControlSplitView>
  );
}

/**
 * 仓库变更分组的记忆化包装。
 *
 * 多仓库工作区里任一仓库状态刷新都会让所有分组重渲染，
 * 而每个分组内部要对全部文件条目重新分组和排序，代价随文件数线性增长。
 */
const RepositoryChangeGroupMemo = memo(RepositoryChangeGroup);

/**
 * 从仓库路径推断显示名。
 *
 * @param root 仓库根路径
 * @returns 末级目录名，取不到时回落为 repository
 */
function fallbackName(root: string): string {
  return root.split(/[\\/]/).filter(Boolean).at(-1) ?? "repository";
}
