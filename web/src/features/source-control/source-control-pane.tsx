import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ArrowDown,
  ArrowUp,
  CloudDownload,
  CloudUpload,
  FolderGit2,
  GitBranch,
  History,
  RefreshCw,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api/client";
import { ApiError, LocalizedError, localizeApiMessage, toDisplayError } from "../../api/api-error";
import type { GitOperationResponse } from "../../api/contracts";
import type { GitOperationOptions } from "../../api/git-contracts";
import { Button } from "../../shared/ui/button/button";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { DiffView } from "../chat/tool-renderers/diff-view";
import { useI18n } from "../i18n/use-i18n";
import { CommitControl } from "./changes/commit-control";
import { groupGitChanges } from "./changes/change-groups";
import { RepositoryChangeGroup } from "./changes/repository-change-group";
import { MoreActionsMenu } from "./actions/more-actions-menu";
import { resolveGitReviewDiffMode } from "./diff/diff-mode";
import { SourceControlDiff } from "./diff/source-control-diff";
import { CommitGraph } from "./graph/commit-graph";
import { formatGitDate } from "./graph/graph-utils";
import { InProgressOperationBar } from "./operation/in-progress-operation-bar";
import { MergeEditor } from "./conflicts/merge-editor";
import { GitOutputPanel } from "./output/git-output-panel";
import { RepositoriesView } from "./repositories/repositories-view";
import { useScmStateStore } from "./state/use-scm-state-store";
import { useGitRepositoryEvents, type GitWatchMode } from "./state/use-git-repository-events";
import { useRepositoryStatuses } from "./state/use-repository-statuses";
import type { GitOutputEntry, GitOperationUiOptions } from "./types";
import "./source-control.css";
import { GitBranchMenu } from "../workspace/git-branch-menu";

/**
 * 渲染 LiveAgent 风格的 Git 变更与历史面板。
 *
 * @returns Git 管理面板
 */
export function SourceControlPane() {
  const confirm = useConfirm();
  const { locale, t } = useI18n();
  const queryClient = useQueryClient();
  const [mode, setMode] = useState<GitWatchMode>("changes");
  const [initBranch, setInitBranch] = useState("main");
  const [remoteUrl, setRemoteUrl] = useState("");
  const [branchMenuOpen, setBranchMenuOpen] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [notice, setNotice] = useState("");
  const [outputEntries, setOutputEntries] = useState<GitOutputEntry[]>([]);
  const [selectedRepoRoot, setSelectedRepoRoot] = useState<string | null>(null);
  const [closedRepoRoots, setClosedRepoRoots] = useState<string[]>([]);
  const scmState = useScmStateStore(selectedRepoRoot);
  const {
    message,
    diffMode,
    selectedPath,
    selectedSection,
    selectedCommit,
    selectedCommitPath,
    historyLimit,
    setMessage,
    setDiffMode,
    setSelectedSection,
    setSelectedCommit,
    setSelectedCommitPath,
    setHistoryLimit,
    selectRepositoryChange
  } = scmState;
  const pendingActionRef = useRef("operation");

  const repositories = useQuery({
    queryKey: ["git-repositories"],
    queryFn: api.workspace.gitRepositories,
    staleTime: 5_000
  });
  const visibleRepositories = useMemo(() => repositories.data ? {
    ...repositories.data,
    repositories: repositories.data.repositories.filter((repository) => !closedRepoRoots.includes(repository.root))
  } : undefined, [closedRepoRoots, repositories.data]);
  const hasRepositories = (repositories.data?.repositories.length ?? 0) > 0;
  const statusRoots = useMemo(() => {
    const roots = (visibleRepositories?.repositories ?? []).map((repository) => repository.root);
    if (selectedRepoRoot && !roots.includes(selectedRepoRoot)) roots.push(selectedRepoRoot);
    return roots;
  }, [selectedRepoRoot, visibleRepositories]);
  const repositoryStatuses = useRepositoryStatuses(
    statusRoots,
    repositories.isSuccess && hasRepositories
  );
  const gitStatus = useQuery({
    queryKey: ["git-status", selectedRepoRoot],
    queryFn: () => api.workspace.gitStatus(selectedRepoRoot ?? undefined),
    enabled: repositories.isSuccess && !hasRepositories
  });
  const state = hasRepositories
    ? repositoryStatuses.data?.repositories.find((repository) => repository.repo_root === selectedRepoRoot)
    : gitStatus.data;
  const statusLoading = hasRepositories ? repositoryStatuses.isLoading : gitStatus.isLoading;
  const statusError = hasRepositories ? repositoryStatuses.error : gitStatus.error;
  const refetchStatus = hasRepositories ? repositoryStatuses.refetch : gitStatus.refetch;
  const gitWatchError = useGitRepositoryEvents(selectedRepoRoot, repositories.isSuccess, mode);
  const branches = useQuery({
    queryKey: ["git-branches", selectedRepoRoot],
    queryFn: () => api.workspace.gitBranches(selectedRepoRoot ?? undefined),
    enabled: state?.status === "ready" && branchMenuOpen,
    staleTime: 10_000
  });
  const history = useQuery({
    queryKey: ["git-log", selectedRepoRoot, historyLimit],
    queryFn: () => api.workspace.gitLog(historyLimit, 0, selectedRepoRoot ?? undefined),
    enabled: state?.status === "ready" && mode === "history",
    staleTime: 10_000
  });
  const commits = history.data?.commits ?? [];
  const activeCommit = selectedCommit ?? commits[0]?.sha ?? null;
  const reviewDiffMode = resolveGitReviewDiffMode(diffMode, selectedSection);
  const reviewDiff = useQuery({
    queryKey: ["git-review-diff", selectedRepoRoot, reviewDiffMode, selectedPath],
    queryFn: () => api.workspace.gitReviewDiff(reviewDiffMode, selectedPath ?? undefined, selectedRepoRoot ?? undefined),
    enabled: state?.status === "ready" && mode === "changes" && selectedSection !== "merge"
  });
  const commitDetails = useQuery({
    queryKey: ["git-commit-details", selectedRepoRoot, activeCommit],
    queryFn: () => api.workspace.gitCommitDetails(activeCommit!, selectedRepoRoot ?? undefined),
    enabled: mode === "history" && Boolean(activeCommit)
  });
  const commitDiff = useQuery({
    queryKey: ["git-commit-diff", selectedRepoRoot, activeCommit, selectedCommitPath],
    queryFn: () => api.workspace.gitCommitDiff(activeCommit!, selectedCommitPath ?? undefined, selectedRepoRoot ?? undefined),
    enabled: mode === "history" && Boolean(activeCommit)
  });

  const ready = state?.status === "ready";
  const groups = useMemo(() => groupGitChanges(state?.entries ?? []), [state?.entries]);
  useEffect(() => {
    const available = (visibleRepositories?.repositories ?? []).flatMap((repository) => [
      repository.root,
      ...repository.worktrees.map((worktree) => worktree.path)
    ]);
    setSelectedRepoRoot((current) => current && available.includes(current) ? current : available[0] ?? null);
  }, [visibleRepositories]);
  useEffect(() => {
    setRemoteUrl(state?.remote_url ?? "");
  }, [state?.remote_url]);
  useEffect(() => {
    setBranchMenuOpen(false);
    setError(null);
    setNotice("");
  }, [selectedRepoRoot]);

  /** 刷新 Git 派生数据；操作响应已携带状态时不重复读取状态。 */
  const refreshAll = async (includeStatus = true) => {
    await Promise.all([
      includeStatus ? queryClient.invalidateQueries({ queryKey: ["git-status"] }) : Promise.resolve(),
      includeStatus ? queryClient.invalidateQueries({ queryKey: ["git-statuses"] }) : Promise.resolve(),
      queryClient.invalidateQueries({ queryKey: ["git-repositories"] }),
      queryClient.invalidateQueries({ queryKey: ["git-branches"] }),
      queryClient.invalidateQueries({ queryKey: ["git-log"] }),
      queryClient.invalidateQueries({ queryKey: ["git-resources"] }),
      queryClient.invalidateQueries({ queryKey: ["git-conflict"] }),
      queryClient.invalidateQueries({ queryKey: ["git-review-diff"] }),
      queryClient.invalidateQueries({ queryKey: ["git-commit-details"] }),
      queryClient.invalidateQueries({ queryKey: ["git-commit-diff"] }),
      queryClient.invalidateQueries({ queryKey: ["workspace-diff"] }),
      queryClient.invalidateQueries({ queryKey: ["file-tree"] })
    ]);
  };

  const op = useMutation({
    mutationFn: (input: { action: string; options: GitOperationOptions }) =>
      api.workspace.gitOp(input.action, input.options),
    onSuccess: async (result, input) => {
      appendOutput(result.ok, result.message, result.stdout, result.stderr);
      queryClient.setQueryData(["git-status", input.options.repo_root ?? selectedRepoRoot], result.state);
      repositoryStatuses.updateRepositoryStatus(result.state);
      if (!result.ok) {
        setError(
          result.message || result.stderr
            ? new ApiError(result.message || result.stderr)
            : new LocalizedError("Git operation failed", "Git 操作失败")
        );
        setNotice("");
        return;
      }
      setError(null);
      setNotice(result.message);
      await refreshAll(false);
    },
    onError: (reason) => {
      const displayError = toDisplayError(reason, "Git operation failed", "Git 操作失败");
      appendOutput(false, displayError.message, "", displayError.message);
      setError(displayError);
      setNotice("");
    }
  });

  /**
   * 将一次 Git 操作输出追加到面板，并限制保留数量。
   *
   * @param ok 操作是否成功
   * @param outputMessage 操作摘要
   * @param stdout Git 标准输出
   * @param stderr Git 标准错误
   * @returns 无返回值
   */
  function appendOutput(ok: boolean, outputMessage: string, stdout: string, stderr: string) {
    setOutputEntries((current) => [
      ...current.slice(-49),
      {
        id: Date.now() * 100 + current.length,
        action: pendingActionRef.current,
        ok,
        message: outputMessage,
        stdout,
        stderr,
        createdAt: Date.now()
      }
    ]);
  }

  const runOp = async (
    action: string,
    options: GitOperationUiOptions = {}
  ): Promise<GitOperationResponse | undefined> => {
    if (options.confirmTitle) {
      const confirmed = await confirm({
        title: options.confirmTitle,
        description: options.confirmDescription ?? t("This action may not be reversible.", "此操作可能无法撤销。"),
        confirmLabel: t("Continue", "继续"),
        danger: true
      });
      if (!confirmed) return undefined;
    }
    setError(null);
    setNotice("");
    const { confirmTitle: _confirmTitle, confirmDescription: _confirmDescription, ...operationOptions } = options;
    pendingActionRef.current = action;
    try {
      return await op.mutateAsync({
        action,
        options: { ...operationOptions, repo_root: operationOptions.repo_root ?? selectedRepoRoot ?? undefined }
      });
    } catch {
      return undefined;
    }
  };

  /**
   * 执行提交变体，并仅在成功后清空提交说明。
   *
   * @param options 提交变体参数
   * @returns 提交是否成功
   */
  const commitChanges = async (options: GitOperationOptions): Promise<boolean> => {
    const result = await runOp("commit", { message, ...options });
    if (!result?.ok) return false;
    setMessage("");
    return true;
  };

  if ((statusLoading || repositories.isLoading) && !state) {
    return (
      <section className="diff-pane git-manager">
        <div className="git-clean">{t("Loading Git status...", "正在读取 Git 状态…")}</div>
      </section>
    );
  }

  const allRepositoriesClosed = hasRepositories && (visibleRepositories?.repositories.length ?? 0) === 0;
  if (allRepositoriesClosed) {
    return (
      <section className="diff-pane git-manager git-review git-repositories-only">
        <RepositoriesView
          data={visibleRepositories}
          loading={repositories.isLoading}
          error={repositories.error}
          busy={op.isPending}
          selectedRoot={null}
          hiddenCount={closedRepoRoots.length}
          onSelect={setSelectedRepoRoot}
          onClose={(root) => setClosedRepoRoots((current) => current.includes(root) ? current : [...current, root])}
          onShowAll={() => setClosedRepoRoots([])}
          onRefresh={() => void refreshAll()}
          runOperation={runOp}
        />
        <GitOutputPanel entries={outputEntries} />
        {gitWatchError && <div className="pane-error">{gitWatchError}</div>}
      </section>
    );
  }

  if (!ready && hasRepositories) {
    return (
      <section className="diff-pane git-manager">
        <div className="git-clean">{t("Loading selected repository...", "正在读取所选仓库…")}</div>
      </section>
    );
  }

  if (!ready) {
    return (
      <section className="diff-pane git-manager">
        <header className="panel-head">
          <div>
            <span className="eyebrow">{t("Git workspace", "Git 工作区")}</span>
            <h2>
              <GitBranch size={15} />
              {t("Version control", "版本管理")}
            </h2>
          </div>
          <Button className="icon-button" onClick={() => void refetchStatus()} aria-label={t("Refresh", "刷新")}>
            <RefreshCw size={14} />
          </Button>
        </header>
        <div className="git-init-panel">
          <GitBranch size={24} />
          <h3>{t("Initialize Git repository", "初始化 Git 仓库")}</h3>
          <p>{t("Create local version history for this workspace and enable future fetch, pull, and push operations.", "为当前工作区创建本地版本历史，并支持后续 fetch / pull / push。")}</p>
          <label>
            <span>{t("Default branch", "默认分支")}</span>
            <input value={initBranch} onChange={(event) => setInitBranch(event.target.value)} spellCheck={false} />
          </label>
          <Button
            variant="primary"
            onClick={() => void runOp("init", { message: initBranch })}
            disabled={!initBranch.trim() || op.isPending}
          >
            {t("Initialize repository", "初始化仓库")}
          </Button>
        </div>
        {(statusError || error) && <div className="pane-error">{error?.message || statusError?.message}</div>}
      </section>
    );
  }

  const busy = op.isPending;
  const dirtyTotal =
    (state?.dirty_counts.staged ?? 0) +
    (state?.dirty_counts.unstaged ?? 0) +
    (state?.dirty_counts.untracked ?? 0) +
    (state?.dirty_counts.conflicted ?? 0);
  const workingCount = groups.changes.length + groups.untracked.length;

  return (
    <section className="diff-pane git-manager git-review">
      <header className="git-review-toolbar">
        <GitBranchMenu
          state={state!}
          branches={branches.data?.branches ?? []}
          loading={branches.isLoading}
          open={branchMenuOpen}
          busy={busy}
          onOpenChange={setBranchMenuOpen}
          onOperation={runOp}
        />
        <div className="git-review-actions">
          <Button className={mode === "changes" ? "active" : ""} onClick={() => setMode("changes")}>
            {t("Changes", "变更")}
          </Button>
          <Button className={mode === "history" ? "active" : ""} onClick={() => setMode("history")}>
            <History size={13} />
            {t("Graph", "提交图")}
          </Button>
          <Button className={mode === "repositories" ? "active" : ""} onClick={() => setMode("repositories")}>
            <FolderGit2 size={13} />
            {t("Repositories", "仓库")}
          </Button>
          <Button disabled={busy} onClick={() => void runOp("fetch")} title={t("Fetch remote updates", "获取远端更新")}>
            <CloudDownload size={13} />
          </Button>
          <Button disabled={busy} onClick={() => void runOp("pull")} title={t("Pull and merge", "拉取并合并")}>
            <RefreshCw size={13} />
          </Button>
          <Button disabled={busy} onClick={() => void runOp("push")} title={t("Push", "推送")}>
            <CloudUpload size={13} />
          </Button>
          <Button disabled={busy} onClick={() => void refreshAll()} title={t("Refresh", "刷新")} aria-label={t("Refresh", "刷新")}>
            <RefreshCw size={13} />
          </Button>
          <MoreActionsMenu
            busy={busy}
            dirtyTotal={dirtyTotal}
            repoRoot={selectedRepoRoot}
            runOperation={runOp}
          />
        </div>
      </header>

      {mode === "changes" ? (
        <div className="git-manager-body">
          <section className="git-change-panel">
            {state?.operation && (
              <InProgressOperationBar
                operation={state.operation}
                conflictedCount={groups.conflicts.length}
                busy={busy}
                runOperation={runOp}
              />
            )}
            <CommitControl
              message={message}
              stagedCount={groups.staged.length}
              workingCount={workingCount}
              conflictedCount={groups.conflicts.length}
              busy={busy}
              onMessageChange={setMessage}
              onCommit={commitChanges}
            />

            <div className="git-diff-mode">
              <Button className={diffMode === "changes" ? "active" : ""} onClick={() => setDiffMode("changes")}>
                {t("Selected changes", "所选变更")}
              </Button>
              <Button className={diffMode === "branch" ? "active" : ""} onClick={() => setDiffMode("branch")}>
                {t("Against baseline", "相对基线")}
              </Button>
              {dirtyTotal > 0 && (
                <Button
                  className="danger"
                  disabled={busy}
                  onClick={() =>
                    void runOp("discard_all", {
                      confirmTitle: t("Discard all changes", "丢弃全部改动"),
                      confirmDescription: t("Discard all staged, unstaged, and untracked changes. This action cannot be undone.", "将放弃所有已暂存、未暂存和未跟踪改动，此操作无法撤销。")
                    })
                  }
                >
                  {t("Discard all", "全部丢弃")}
                </Button>
              )}
            </div>

            <div className="git-repository-change-list">
              {(hasRepositories ? repositoryStatuses.data?.repositories ?? [] : [state!]).map((repository) => {
                const summary = visibleRepositories?.repositories.find((item) => item.root === repository.repo_root);
                const name = summary?.name ?? repository.repo_root.split(/[\\/]/).filter(Boolean).at(-1) ?? "repository";
                return (
                  <RepositoryChangeGroup
                    key={repository.repo_root}
                    name={name}
                    state={repository}
                    active={repository.repo_root === selectedRepoRoot || !hasRepositories}
                    selectedPath={selectedPath}
                    busy={busy}
                    runOperation={runOp}
                    onSelectRepository={() => setSelectedRepoRoot(repository.repo_root)}
                    onSelectChange={(path, section) => {
                      selectRepositoryChange(repository.repo_root, path, section);
                      setSelectedRepoRoot(repository.repo_root);
                    }}
                  />
                );
              })}
            </div>

            <div className="git-remote-box">
              <span>{state?.remote_url ? t("Remote origin", "远端 origin") : t("Set origin remote", "设置 origin 远端")}</span>
              <input
                value={remoteUrl}
                onChange={(event) => setRemoteUrl(event.target.value)}
                placeholder="git@github.com:org/repo.git"
                spellCheck={false}
              />
              <Button
                disabled={!remoteUrl.trim() || busy}
                onClick={() => void runOp("set_remote", { remote_url: remoteUrl })}
              >
                {state?.remote_url ? t("Update remote", "更新远端") : t("Save remote", "保存远端")}
              </Button>
            </div>
          </section>

          <div className="diff-scroll">
            {selectedSection === "merge" && selectedPath ? (
              <MergeEditor
                path={selectedPath}
                repoRoot={selectedRepoRoot}
                busy={busy}
                runOperation={runOp}
                onResolved={() => setSelectedSection("staged")}
              />
            ) : (
              <SourceControlDiff
                data={reviewDiff.data}
                loading={reviewDiff.isLoading}
                error={reviewDiff.error}
                selectedPath={selectedPath}
                busy={busy}
                runOperation={runOp}
              />
            )}
          </div>
        </div>
      ) : mode === "history" ? (
        <div className="git-manager-body">
          <section className="git-history-panel">
            <div className="git-change-head">
              <span>{t(`Source Control Graph ${commits.length}`, `源代码管理提交图 ${commits.length}`)}</span>
              {(history.data?.history_ahead || history.data?.history_behind) ? (
                <small className="git-history-sync">
                  <span><ArrowUp size={10} />{history.data?.history_ahead ?? 0}</span>
                  <span><ArrowDown size={10} />{history.data?.history_behind ?? 0}</span>
                </small>
              ) : null}
            </div>
            <CommitGraph
              commits={commits}
              activeCommit={activeCommit}
              busy={busy}
              locale={locale}
              canLoadMore={commits.length >= historyLimit}
              onSelect={(commit) => {
                setSelectedCommit(commit.sha);
                setSelectedCommitPath(null);
              }}
              onLoadMore={() => setHistoryLimit((value) => value + 40)}
              runOperation={runOp}
            />
          </section>
          <div className="diff-scroll">
            {activeCommit && commitDetails.data ? (
              <div className="git-diff-shell">
                <div className="git-commit-meta">
                  <h3>{commitDetails.data.commit.subject}</h3>
                  <p>
                    {commitDetails.data.commit.short_sha} · {commitDetails.data.commit.author_name} ·{" "}
                    {formatGitDate(commitDetails.data.commit.author_date, locale)}
                  </p>
                  {commitDetails.data.commit.body && <pre>{commitDetails.data.commit.body}</pre>}
                  <div className="git-commit-files">
                    {commitDetails.data.commit.files.map((file) => (
                      <Button
                        key={`${file.status}:${file.path}`}
                        className={selectedCommitPath === file.path ? "active" : ""}
                        onClick={() => setSelectedCommitPath(file.path)}
                      >
                        <span>{file.status}</span>
                        <strong>{file.path}</strong>
                      </Button>
                    ))}
                  </div>
                </div>
                {commitDiff.data?.patch ? (
                  <>
                    {commitDiff.data.stat && <pre className="git-diff-stat">{commitDiff.data.stat}</pre>}
                    <DiffView source={commitDiff.data.patch} headerPath={selectedCommitPath ?? undefined} />
                  </>
                ) : (
                  <div className="git-clean">{t("No commit diff to display", "没有可显示的提交差异")}</div>
                )}
              </div>
            ) : (
              <div className="git-clean diff-clean">{t("Select a commit to view details", "选择一条提交查看详情")}</div>
            )}
          </div>
        </div>
      ) : (
        <RepositoriesView
          data={visibleRepositories}
          loading={repositories.isLoading}
          error={repositories.error}
          busy={busy}
          selectedRoot={selectedRepoRoot}
          hiddenCount={closedRepoRoots.length}
          onSelect={setSelectedRepoRoot}
          onClose={(root) => setClosedRepoRoots((current) => current.includes(root) ? current : [...current, root])}
          onShowAll={() => setClosedRepoRoots([])}
          onRefresh={() => void refreshAll()}
          runOperation={runOp}
        />
      )}

      <GitOutputPanel entries={outputEntries} />
      {(error || gitWatchError || notice || statusError) && (
        <div className={error || gitWatchError || statusError ? "pane-error" : "pane-notice"}>
          {error?.message || gitWatchError || statusError?.message || localizeApiMessage(notice, locale)}
        </div>
      )}
    </section>
  );
}
