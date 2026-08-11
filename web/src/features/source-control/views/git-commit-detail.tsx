import { ExternalLink } from "lucide-react";
import type { GitCommitDetailsResponse, GitDiffResponse } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { DiffView } from "../../chat/tool-renderers/diff-view";
import { useI18n } from "../../i18n/use-i18n";
import { GitDiffStat } from "../diff/git-diff-stat";
import { formatGitDate } from "../graph/graph-utils";
import { gitHubCommitUrl } from "../links/github-url";

type GitCommitDetailProps = {
  details?: GitCommitDetailsResponse;
  diff?: GitDiffResponse;
  selectedPath: string | null;
  onSelectPath: (path: string) => void;
  locale: "en-US" | "zh-CN";
};

/**
 * 渲染单条提交的元信息、文件列表与差异。
 *
 * @param props 提交详情、差异数据与文件选择状态
 * @returns 提交详情区；未选中提交时返回空态提示
 */
export function GitCommitDetail({ details, diff, selectedPath, onSelectPath, locale }: GitCommitDetailProps) {
  const { t } = useI18n();
  if (!details) {
    return <div className="git-clean diff-clean">{t("Select a commit to view details", "选择一条提交查看详情")}</div>;
  }
  const commit = details.commit;
  // 远端指向 GitHub 时给出网页入口，其他托管商留空不渲染
  const commitUrl = gitHubCommitUrl(commit.remote_url, commit.sha);
  return (
    <div className="git-diff-shell">
      <div className="git-commit-meta">
        <h3>{commit.subject}</h3>
        <p>
          {commit.short_sha} · {commit.author_name} · {formatGitDate(commit.author_date, locale)}
          {commitUrl && (
            <a className="git-commit-link" href={commitUrl} target="_blank" rel="noreferrer noopener">
              <ExternalLink size={11} />
              {t("View on GitHub", "在 GitHub 查看")}
            </a>
          )}
        </p>
        {commit.body && <pre>{commit.body}</pre>}
        <div className="git-commit-files">
          {commit.files.map((file) => (
            <Button
              key={`${file.status}:${file.path}`}
              className={selectedPath === file.path ? "active" : ""}
              onClick={() => onSelectPath(file.path)}
            >
              <span>{file.status}</span>
              <strong>{file.path}</strong>
            </Button>
          ))}
        </div>
      </div>
      {diff?.patch ? (
        <>
          {diff.stat ? <GitDiffStat stat={diff.stat} /> : null}
          <DiffView source={diff.patch} headerPath={selectedPath ?? undefined} />
        </>
      ) : (
        <div className="git-clean">{t("No commit diff to display", "没有可显示的提交差异")}</div>
      )}
    </div>
  );
}
