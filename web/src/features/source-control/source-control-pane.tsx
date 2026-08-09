import { useQuery } from "@tanstack/react-query";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../../api/client";
import { localizeApiMessage, toDisplayError } from "../../api/api-error";
import type { GitOperationAction, GitOperationOptions } from "../../api/git-contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../i18n/use-i18n";
import { switchWithTerminalConfirm } from "../workspaces/workspace-switcher";
import { groupGitChanges } from "./changes/change-groups";
import { resolveGitReviewDiffMode } from "./diff/diff-mode";
import { useFileComparison } from "./diff/use-file-comparison";
import type { CloneRepositoryInput } from "./empty/clone-repository-dialog";
import { GitOperationToast } from "./output/git-operation-toast";
import { GitOutputPanel } from "./output/git-output-panel";
import { buildOperationNotice, type OperationNotice } from "./output/operation-notice";
import { RemoteSetupDialog } from "./remote/remote-setup-dialog";
import { shouldPromptRemoteSetup, type RemoteDependentAction } from "./remote/remote-setup-trigger";
import { RepositoriesView } from "./repositories/repositories-view";
import { GitSetupView } from "./shell/git-setup-view";
import { GitToolbar } from "./shell/git-toolbar";
import { useScmStateStore } from "./state/use-scm-state-store";
import { useGitRepositoryEvents, type GitWatchMode } from "./state/use-git-repository-events";
import { useGitSettings } from "./state/use-git-settings";
import { useGitAutofetch } from "./state/use-git-autofetch";
import { useGitOperations } from "./state/use-git-operations";
import { useGitWorkspace } from "./state/use-git-workspace";
import { resolveScmCountBadge } from "./state/scm-count-badge";
import type { RunGitOperation } from "./types";
import { GitChangesView } from "./views/git-changes-view";
import { GitHistoryView } from "./views/git-history-view";
import "./source-control.css";

/**
 * 渲染 Git 变更、提交图与仓库管理面板。
 *
 * 本组件只负责装配：仓库状态与操作分别由 useGitWorkspace / useGitOperations 提供，
 * 三种视图各自成组件，这里只做模式路由与跨视图数据的串联。
 *
 * @returns Git 管理面板
 */
export function SourceControlPane() {
  const confirm = useConfirm();
  const { locale, t } = useI18n();
  const [mode, setMode] = useState<GitWatchMode>("changes");
  const [initBranch, setInitBranch] = useState("main");
  const [remoteUrl, setRemoteUrl] = useState("");
  const [branchMenuOpen, setBranchMenuOpen] = useState(false);
  const [openFolderDialogOpen, setOpenFolderDialogOpen] = useState(false);
  const [cloneDialogOpen, setCloneDialogOpen] = useState(false);
  const [cloneInput, setCloneInput] = useState<CloneRepositoryInput | null>(null);
  const [suggestingMessage, setSuggestingMessage] = useState(false);

  const { scm, git } = useGitSettings();
  const workspace = useGitWorkspace();
  const { state, selectedRoot, hasRepositories } = workspace;
  const scmState = useScmStateStore(selectedRoot);
  const fileComparison = useFileComparison();

  const operations = useGitOperations({
    selectedRoot,
    onStateUpdated: workspace.repositoryStatuses.updateRepositoryStatus,
  });
  const { runOperation: runRawOperation, busy, error, setError, notice, setNotice } = operations;

  const [operationNotice, setOperationNotice] = useState<OperationNotice | null>(null);
  const [remoteSetupAction, setRemoteSetupAction] = useState<RemoteDependentAction | null>(null);
  const [remoteSetupUrl, setRemoteSetupUrl] = useState("");
  const [remoteSetupError, setRemoteSetupError] = useState("");
  const noticeSeqRef = useRef(0);

  /**
   * 执行 Git 操作并把结果转成浮出提示。
   *
   * 底部错误行与输出面板保持原样，这里只额外补一条短反馈；
   * 远端相关操作因缺少远端而失败时，改为弹出配置引导。
   *
   * @param action 操作标识
   * @param options 操作参数
   * @returns 操作响应；被取消或失败时为 undefined
   */
  const runOperation = useCallback<RunGitOperation>(
    async (action, options) => {
      const result = await runRawOperation(action, options);
      if (!result) return result;

      // 1. 缺少远端时不提示失败，直接引导用户补配远端
      const failureText = result.message || result.stderr;
      if (!result.ok && shouldPromptRemoteSetup(action, failureText)) {
        setRemoteSetupAction(action as RemoteDependentAction);
        setRemoteSetupError("");
        setOperationNotice(null);
        return result;
      }

      // 2. 其余结果统一浮出成功或失败提示
      noticeSeqRef.current += 1;
      setOperationNotice(
        buildOperationNotice(noticeSeqRef.current, result.ok ? "success" : "error", action, result.ok ? result.message : failureText)
      );
      return result;
    },
    [runRawOperation]
  );

  /**
   * 保存远端地址，成功后重试触发引导的那次操作。
   *
   * @returns 无
   */
  const saveRemoteAndRetry = useCallback(async () => {
    const action = remoteSetupAction;
    if (!action) return;
    setRemoteSetupError("");

    // 1. 先写入远端地址，失败则把原因留在弹层内
    const saved = await runRawOperation("set_remote", { remote_url: remoteSetupUrl });
    if (!saved?.ok) {
      setRemoteSetupError(saved?.message || saved?.stderr || t("Failed to save the remote", "保存远端失败"));
      return;
    }

    // 2. 远端就绪后关闭弹层并重试原操作
    setRemoteSetupAction(null);
    setRemoteUrl(remoteSetupUrl);
    await runOperation(action);
  }, [remoteSetupAction, remoteSetupUrl, runOperation, runRawOperation, t]);

  const gitWatchError = useGitRepositoryEvents(selectedRoot, workspace.repositories.isSuccess, mode);
  const ready = state?.status === "ready";

  const branches = useQuery({
    queryKey: ["git-branches", selectedRoot],
    queryFn: () => api.workspace.gitBranches(selectedRoot ?? undefined),
    enabled: ready && branchMenuOpen,
    staleTime: 10_000,
  });
  const history = useQuery({
    queryKey: ["git-log", selectedRoot, scmState.historyLimit],
    queryFn: () => api.workspace.gitLog(scmState.historyLimit, 0, selectedRoot ?? undefined),
    enabled: ready && mode === "history",
    staleTime: 10_000,
  });
  const activeCommit = scmState.selectedCommit ?? history.data?.commits[0]?.sha ?? null;
  const reviewDiffMode = resolveGitReviewDiffMode(scmState.diffMode, scmState.selectedSection);
  const reviewDiff = useQuery({
    queryKey: ["git-review-diff", selectedRoot, reviewDiffMode, scmState.selectedPath],
    queryFn: () =>
      api.workspace.gitReviewDiff(reviewDiffMode, scmState.selectedPath ?? undefined, selectedRoot ?? undefined),
    enabled: ready && mode === "changes" && scmState.selectedSection !== "merge" && !fileComparison.target,
  });
  const commitDetails = useQuery({
    queryKey: ["git-commit-details", selectedRoot, activeCommit],
    queryFn: () => api.workspace.gitCommitDetails(activeCommit!, selectedRoot ?? undefined),
    enabled: mode === "history" && Boolean(activeCommit),
    staleTime: 30_000,
  });
  const commitDiff = useQuery({
    queryKey: ["git-commit-diff", selectedRoot, activeCommit, scmState.selectedCommitPath],
    queryFn: () =>
      api.workspace.gitCommitDiff(activeCommit!, scmState.selectedCommitPath ?? undefined, selectedRoot ?? undefined),
    enabled: mode === "history" && Boolean(activeCommit),
    staleTime: 30_000,
  });

  const groups = useMemo(
    () => groupGitChanges(state?.entries ?? [], git.untracked_changes),
    [git.untracked_changes, state?.entries]
  );
  const repositoryNames = useMemo(() => {
    const names = new Map<string, string>();
    for (const repository of workspace.visibleRepositories?.repositories ?? []) {
      names.set(repository.root, repository.name);
    }
    return names;
  }, [workspace.visibleRepositories]);

  useEffect(() => {
    setRemoteUrl(state?.remote_url ?? "");
  }, [state?.remote_url]);
  useEffect(() => {
    setBranchMenuOpen(false);
    setError(null);
    setNotice("");
    setOperationNotice(null);
    setRemoteSetupAction(null);
  }, [selectedRoot, setError, setNotice]);

  useEffect(() => {
    // 引导弹层打开时预填当前远端地址，多数情况下用户只需确认
    if (remoteSetupAction) setRemoteSetupUrl(state?.remote_url ?? "");
  }, [remoteSetupAction, state?.remote_url]);

  /**
   * 登记服务端目录并切换当前工作区。
   *
   * @param path 待打开目录绝对路径
   * @returns 无
   */
  const openWorkspace = async (path: string) => {
    const created = await api.workspaces.add(path);
    const switched = await switchWithTerminalConfirm(created.id, confirm, t);
    if (switched) window.location.reload();
  };

  /**
   * 克隆仓库到选中的父目录，并在完成后询问是否打开。
   *
   * @param parent 目标父目录绝对路径
   * @returns 无
   */
  const cloneRepository = async (parent: string) => {
    if (!cloneInput) return;
    operations.setPendingAction("clone");
    setError(null);
    setNotice("");
    let outputRecorded = false;
    try {
      // 1. 调用独立 clone 接口，保留真实 Git 输出
      const result = await operations.clone.mutateAsync({ ...cloneInput, parent });
      operations.appendOutput(result.ok, result.message, result.stdout, result.stderr);
      outputRecorded = true;
      if (!result.ok) throw new Error(result.message || result.stderr);

      // 2. 关闭目标目录弹层并刷新仓库发现
      setCloneInput(null);
      setNotice(result.message);
      await operations.refreshAll();

      // 3. 克隆完成后由用户决定是否切换到新仓库
      const shouldOpen = await confirm({
        title: t("Open cloned repository?", "打开已克隆仓库？"),
        description: t(
          `Repository cloned to ${result.state.repo_root}.`,
          `仓库已克隆到 ${result.state.repo_root}。`
        ),
        confirmLabel: t("Open Repository", "打开仓库"),
      });
      if (shouldOpen) await openWorkspace(result.state.repo_root);
    } catch (reason) {
      const displayError = toDisplayError(reason, "Failed to clone repository", "克隆仓库失败");
      if (!outputRecorded) operations.appendOutput(false, displayError.message, "", displayError.message);
      setError(displayError);
      setNotice("");
      throw displayError;
    }
  };

  /**
   * 调用小模型根据当前仓库改动生成提交说明。
   *
   * @returns 无
   */
  const suggestCommitMessage = async () => {
    if (suggestingMessage || busy) return;
    setSuggestingMessage(true);
    try {
      const result = await api.workspace.suggestCommitMessage(selectedRoot || undefined);
      if (result.message?.trim()) scmState.setMessage(result.message.trim());
    } catch (reason) {
      setNotice(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSuggestingMessage(false);
    }
  };

  /**
   * 执行提交，仅在成功后清空提交说明。
   *
   * @param options 提交变体参数
   * @returns 提交是否成功
   */
  const commitChanges = async (options: GitOperationOptions): Promise<boolean> => {
    const result = await runOperation("commit", { message: scmState.message, ...options });
    if (!result?.ok) return false;
    scmState.setMessage("");
    return true;
  };

  useGitAutofetch({
    enabled: git.autofetch,
    ready: Boolean(ready),
    remoteConfigured: Boolean(state?.remote_url),
    busy,
    hasOperation: Boolean(state?.operation),
    onFetch: () => runOperation("fetch"),
  });

  if ((workspace.statusLoading || workspace.repositories.isLoading) && !state) {
    return (
      <section className="diff-pane git-manager">
        <div className="git-clean">{t("Loading Git status...", "正在读取 Git 状态…")}</div>
      </section>
    );
  }

  const allRepositoriesClosed =
    hasRepositories && (workspace.visibleRepositories?.repositories.length ?? 0) === 0;
  if (allRepositoriesClosed) {
    return (
      <section className="diff-pane git-manager git-review git-repositories-only">
        <RepositoriesView
          data={workspace.visibleRepositories}
          loading={workspace.repositories.isLoading}
          error={workspace.repositories.error}
          busy={busy}
          selectedRoot={null}
          hiddenCount={workspace.closedRoots.length}
          onSelect={workspace.setSelectedRoot}
          onClose={workspace.closeRepository}
          onShowAll={workspace.showAllRepositories}
          onRefresh={() => void operations.refreshAll()}
          runOperation={runOperation}
        />
        <GitOutputPanel entries={operations.outputEntries} />
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

  if (!ready || !state) {
    return (
      <GitSetupView
        initBranch={initBranch}
        onInitBranchChange={setInitBranch}
        busy={busy}
        onInitialize={() => void runOperation("init", { message: initBranch })}
        onRefresh={() => void workspace.refetchStatus()}
        outputEntries={operations.outputEntries}
        errorMessage={error?.message ?? workspace.statusError?.message ?? null}
        cloneDialogOpen={cloneDialogOpen}
        onCloneDialogOpenChange={setCloneDialogOpen}
        onCloneContinue={(input) => {
          setCloneDialogOpen(false);
          setCloneInput(input);
        }}
        cloneTargetOpen={Boolean(cloneInput)}
        onCloneTargetClose={() => setCloneInput(null)}
        onCloneTargetSelect={cloneRepository}
        openFolderDialogOpen={openFolderDialogOpen}
        onOpenFolderDialogOpenChange={setOpenFolderDialogOpen}
        onOpenFolder={openWorkspace}
      />
    );
  }

  const dirtyTotal =
    state.dirty_counts.staged +
    state.dirty_counts.unstaged +
    state.dirty_counts.untracked +
    state.dirty_counts.conflicted;
  const countBadge = resolveScmCountBadge(
    scm.count_badge,
    workspace.allStates,
    state.repo_root ?? selectedRoot,
    git.untracked_changes
  );

  return (
    <section className="diff-pane git-manager git-review">
      <GitOperationToast notice={operationNotice} onDismiss={() => setOperationNotice(null)} />
      {remoteSetupAction && (
        <RemoteSetupDialog
          open
          action={remoteSetupAction}
          workdir={state.repo_root ?? selectedRoot ?? ""}
          branch={state.head || t("Unresolved", "未确定")}
          remoteUrl={remoteSetupUrl}
          busy={busy}
          error={remoteSetupError}
          onRemoteUrlChange={setRemoteSetupUrl}
          onClose={() => setRemoteSetupAction(null)}
          onSubmit={() => void saveRemoteAndRetry()}
        />
      )}
      <GitToolbar
        state={state}
        branches={branches.data?.branches ?? []}
        branchesLoading={branches.isLoading}
        branchMenuOpen={branchMenuOpen}
        onBranchMenuOpenChange={setBranchMenuOpen}
        suggestBranchNames={git.branch_random_name.enable}
        mode={mode}
        onModeChange={setMode}
        countBadge={countBadge}
        busy={busy}
        dirtyTotal={dirtyTotal}
        repoRoot={selectedRoot}
        confirmSync={git.confirm_sync}
        confirmForcePush={git.confirm_force_push}
        onRefresh={() => void operations.refreshAll()}
        runOperation={runOperation}
      />

      {mode === "changes" && (
        <GitChangesView
          state={state}
          allStates={workspace.allStates}
          repositoryNames={repositoryNames}
          selectedRoot={selectedRoot}
          onSelectRoot={workspace.setSelectedRoot}
          scm={scm}
          git={git}
          busy={busy}
          dirtyTotal={dirtyTotal}
          scmState={scmState}
          fileComparison={fileComparison}
          conflictCount={groups.conflicts.length}
          stagedCount={groups.staged.length}
          workingCount={groups.changes.length + groups.untracked.length}
          remoteUrl={remoteUrl}
          onRemoteUrlChange={setRemoteUrl}
          reviewDiff={{ data: reviewDiff.data, isLoading: reviewDiff.isLoading, error: reviewDiff.error }}
          suggestingMessage={suggestingMessage}
          onSuggestMessage={() => void suggestCommitMessage()}
          onCommit={commitChanges}
          runOperation={runOperation}
        />
      )}

      {mode === "history" && (
        <GitHistoryView
          history={history.data}
          activeCommit={activeCommit}
          onSelectCommit={(sha) => {
            scmState.setSelectedCommit(sha);
            scmState.setSelectedCommitPath(null);
          }}
          historyLimit={scmState.historyLimit}
          onLoadMore={() => scmState.setHistoryLimit((value) => value + 40)}
          details={commitDetails.data}
          diff={commitDiff.data}
          selectedCommitPath={scmState.selectedCommitPath}
          onSelectCommitPath={scmState.setSelectedCommitPath}
          suggestBranchNames={git.branch_random_name.enable}
          busy={busy}
          locale={locale}
          runOperation={runOperation}
        />
      )}

      {mode === "repositories" && (
        <RepositoriesView
          data={workspace.visibleRepositories}
          loading={workspace.repositories.isLoading}
          error={workspace.repositories.error}
          busy={busy}
          selectedRoot={selectedRoot}
          hiddenCount={workspace.closedRoots.length}
          onSelect={workspace.setSelectedRoot}
          onClose={workspace.closeRepository}
          onShowAll={workspace.showAllRepositories}
          onRefresh={() => void operations.refreshAll()}
          runOperation={runOperation}
        />
      )}

      <GitOutputPanel entries={operations.outputEntries} />
      {(error || gitWatchError || notice || workspace.statusError) && (
        <div className={error || gitWatchError || workspace.statusError ? "pane-error" : "pane-notice"}>
          {error?.message || gitWatchError || workspace.statusError?.message || localizeApiMessage(notice, locale)}
        </div>
      )}
    </section>
  );
}
