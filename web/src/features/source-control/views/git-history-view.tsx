import { ArrowDown, ArrowUp } from "lucide-react";
import type { GitCommitDetailsResponse, GitDiffResponse, GitLogResponse } from "../../../api/contracts";
import { useI18n } from "../../i18n/use-i18n";
import { CommitGraph } from "../graph/commit-graph";
import { SourceControlSplitView } from "../layout/source-control-split-view";
import type { RunGitOperation } from "../types";
import { GitCommitDetail } from "./git-commit-detail";
import "./git-views.css";

type GitHistoryViewProps = {
  history?: GitLogResponse;
  activeCommit: string | null;
  onSelectCommit: (sha: string) => void;
  historyLimit: number;
  onLoadMore: () => void;
  details?: GitCommitDetailsResponse;
  diff?: GitDiffResponse;
  selectedCommitPath: string | null;
  onSelectCommitPath: (path: string) => void;
  suggestBranchNames: boolean;
  busy: boolean;
  locale: "en-US" | "zh-CN";
  runOperation: RunGitOperation;
};

/**
 * 渲染提交图视图：左侧提交列表，右侧提交详情。
 *
 * @param props 提交历史、选中提交与操作回调
 * @returns 提交图视图
 */
export function GitHistoryView(props: GitHistoryViewProps) {
  const { t } = useI18n();
  const commits = props.history?.commits ?? [];
  const ahead = props.history?.history_ahead ?? 0;
  const behind = props.history?.history_behind ?? 0;
  return (
    <SourceControlSplitView className="git-history-body">
      <section className="git-history-panel">
        <div className="git-change-head">
          <span>{t(`Commit graph · ${commits.length}`, `提交图 · ${commits.length}`)}</span>
          {(ahead > 0 || behind > 0) && (
            <small className="git-history-sync">
              <span>
                <ArrowUp size={10} />
                {ahead}
              </span>
              <span>
                <ArrowDown size={10} />
                {behind}
              </span>
            </small>
          )}
        </div>
        <CommitGraph
          commits={commits}
          activeCommit={props.activeCommit}
          busy={props.busy}
          locale={props.locale}
          canLoadMore={commits.length >= props.historyLimit}
          suggestBranchNames={props.suggestBranchNames}
          onSelect={(commit) => props.onSelectCommit(commit.sha)}
          onLoadMore={props.onLoadMore}
          runOperation={props.runOperation}
        />
      </section>
      <div className="diff-scroll">
        <GitCommitDetail
          details={props.activeCommit ? props.details : undefined}
          diff={props.diff}
          selectedPath={props.selectedCommitPath}
          onSelectPath={props.onSelectCommitPath}
          locale={props.locale}
        />
      </div>
    </SourceControlSplitView>
  );
}
