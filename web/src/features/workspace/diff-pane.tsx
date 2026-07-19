import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Archive,
  ArrowDown,
  ArrowUp,
  Check,
  ChevronDown,
  CloudDownload,
  CloudUpload,
  EyeOff,
  GitBranch,
  GitCommitHorizontal,
  History,
  Minus,
  Plus,
  RefreshCw,
  RotateCcw,
  Trash2
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api } from "../../api/client";
import { ApiError, LocalizedError, localizeApiMessage, toDisplayError } from "../../api/api-error";
import type { GitCommitSummary, GitOperationResponse, GitStatusEntry } from "../../api/contracts";
import type { GitOperationOptions } from "../../api/git-contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { DiffView } from "../chat/tool-renderers/diff-view";
import { useI18n } from "../i18n/use-i18n";
import { GitBranchMenu } from "./git-branch-menu";

type ReviewMode = "changes" | "history";
type DiffMode = "working_tree" | "branch";

/**
 * 渲染 LiveAgent 风格的 Git 变更与历史面板。
 *
 * @returns Git 管理面板
 */
export function DiffPane() {
  const confirm = useConfirm();
  const { locale, t } = useI18n();
  const queryClient = useQueryClient();
  const [mode, setMode] = useState<ReviewMode>("changes");
  const [diffMode, setDiffMode] = useState<DiffMode>("working_tree");
  const [message, setMessage] = useState("");
  const [initBranch, setInitBranch] = useState("main");
  const [remoteUrl, setRemoteUrl] = useState("");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [selectedCommit, setSelectedCommit] = useState<string | null>(null);
  const [selectedCommitPath, setSelectedCommitPath] = useState<string | null>(null);
  const [historyLimit, setHistoryLimit] = useState(40);
  const [branchMenuOpen, setBranchMenuOpen] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const [notice, setNotice] = useState("");

  const status = useQuery({
    queryKey: ["git-status"],
    queryFn: api.workspace.gitStatus,
    refetchInterval: 8_000
  });
  const branches = useQuery({
    queryKey: ["git-branches"],
    queryFn: api.workspace.gitBranches,
    enabled: status.data?.status === "ready" && branchMenuOpen,
    staleTime: 10_000
  });
  const history = useQuery({
    queryKey: ["git-log", historyLimit],
    queryFn: () => api.workspace.gitLog(historyLimit, 0),
    enabled: status.data?.status === "ready" && mode === "history",
    staleTime: 10_000
  });
  const reviewDiff = useQuery({
    queryKey: ["git-review-diff", diffMode, selectedPath],
    queryFn: () => api.workspace.gitReviewDiff(diffMode, selectedPath ?? undefined),
    enabled: status.data?.status === "ready" && mode === "changes"
  });
  const commitDetails = useQuery({
    queryKey: ["git-commit-details", selectedCommit],
    queryFn: () => api.workspace.gitCommitDetails(selectedCommit!),
    enabled: mode === "history" && Boolean(selectedCommit)
  });
  const commitDiff = useQuery({
    queryKey: ["git-commit-diff", selectedCommit, selectedCommitPath],
    queryFn: () => api.workspace.gitCommitDiff(selectedCommit!, selectedCommitPath ?? undefined),
    enabled: mode === "history" && Boolean(selectedCommit)
  });

  const state = status.data;
  const ready = state?.status === "ready";
  const staged = useMemo(
    () => (state?.entries ?? []).filter((entry) => entry.staged && !entry.untracked),
    [state?.entries]
  );
  const changes = useMemo(
    () =>
      (state?.entries ?? []).filter(
        (entry) => entry.untracked || entry.worktree_status !== "." || entry.conflicted
      ),
    [state?.entries]
  );
  useEffect(() => {
    if (state?.remote_url) setRemoteUrl(state.remote_url);
  }, [state?.remote_url]);

  /** 刷新 Git 派生数据；操作响应已携带状态时不重复读取状态。 */
  const refreshAll = async (includeStatus = true) => {
    await Promise.all([
      includeStatus ? queryClient.invalidateQueries({ queryKey: ["git-status"] }) : Promise.resolve(),
      queryClient.invalidateQueries({ queryKey: ["git-branches"] }),
      queryClient.invalidateQueries({ queryKey: ["git-log"] }),
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
    onSuccess: async (result) => {
      queryClient.setQueryData(["git-status"], result.state);
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
      setError(toDisplayError(reason, "Git operation failed", "Git 操作失败"));
      setNotice("");
    }
  });

  const runOp = async (
    action: string,
    options: GitOperationOptions & {
      confirmTitle?: string;
      confirmDescription?: string;
    } = {}
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
    const result = await op.mutateAsync({ action, options: operationOptions });
    if (action === "commit") setMessage("");
    return result;
  };

  if (status.isLoading && !state) {
    return (
      <section className="diff-pane git-manager">
        <div className="git-clean">{t("Loading Git status...", "正在读取 Git 状态…")}</div>
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
          <button type="button" className="icon-button" onClick={() => void status.refetch()} aria-label={t("Refresh", "刷新")}>
            <RefreshCw size={14} />
          </button>
        </header>
        <div className="git-init-panel">
          <GitBranch size={24} />
          <h3>{t("Initialize Git repository", "初始化 Git 仓库")}</h3>
          <p>{t("Create local version history for this workspace and enable future fetch, pull, and push operations.", "为当前工作区创建本地版本历史，并支持后续 fetch / pull / push。")}</p>
          <label>
            <span>{t("Default branch", "默认分支")}</span>
            <input value={initBranch} onChange={(event) => setInitBranch(event.target.value)} spellCheck={false} />
          </label>
          <button
            type="button"
            onClick={() => void runOp("init", { message: initBranch })}
            disabled={!initBranch.trim() || op.isPending}
          >
            {t("Initialize repository", "初始化仓库")}
          </button>
        </div>
        {(status.error || error) && <div className="pane-error">{error?.message || status.error?.message}</div>}
      </section>
    );
  }

  const busy = op.isPending;
  const commits = history.data?.commits ?? [];
  const activeCommit = selectedCommit ?? commits[0]?.sha ?? null;
  const dirtyTotal =
    (state?.dirty_counts.staged ?? 0) +
    (state?.dirty_counts.unstaged ?? 0) +
    (state?.dirty_counts.untracked ?? 0) +
    (state?.dirty_counts.conflicted ?? 0);

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
          <button type="button" className={mode === "changes" ? "active" : ""} onClick={() => setMode("changes")}>
            {t("Changes", "变更")}
          </button>
          <button type="button" className={mode === "history" ? "active" : ""} onClick={() => setMode("history")}>
            <History size={13} />
            {t("History", "历史")}
          </button>
          <button type="button" disabled={busy} onClick={() => void runOp("fetch")} title={t("Fetch remote updates", "获取远端更新")}>
            <CloudDownload size={13} />
          </button>
          <button type="button" disabled={busy} onClick={() => void runOp("pull")} title={t("Pull and merge", "拉取并合并")}>
            <RefreshCw size={13} />
          </button>
          <button type="button" disabled={busy} onClick={() => void runOp("push")} title={t("Push", "推送")}>
            <CloudUpload size={13} />
          </button>
          <button
            type="button"
            disabled={busy || dirtyTotal === 0}
            onClick={() => void runOp("stash_push", { message: "Sai stash" })}
            title={t("Stash changes", "暂存修改")}
          >
            <Archive size={13} />
          </button>
          {(state?.stash_count ?? 0) > 0 && (
            <button type="button" disabled={busy} onClick={() => void runOp("stash_pop")} title={t(`Pop stash (${state?.stash_count})`, `弹出 stash (${state?.stash_count})`)}>
              {t("Pop", "弹出")}
            </button>
          )}
          <button type="button" disabled={busy} onClick={() => void refreshAll()} title={t("Refresh", "刷新")} aria-label={t("Refresh", "刷新")}>
            <RefreshCw size={13} />
          </button>
        </div>
      </header>

      {mode === "changes" ? (
        <div className="git-manager-body">
          <section className="git-change-panel">
            <div className="git-commit-box">
              <textarea rows={3} value={message} onChange={(event) => setMessage(event.target.value)} placeholder={t("Commit message", "提交说明")} />
              <button
                type="button"
                onClick={() => void runOp("commit", { message })}
                disabled={!message.trim() || busy || (state?.dirty_counts.staged ?? 0) === 0}
              >
                <Check size={13} />
                {t("Commit staged changes", "提交已暂存变更")}
              </button>
            </div>

            <div className="git-diff-mode">
              <button type="button" className={diffMode === "working_tree" ? "active" : ""} onClick={() => setDiffMode("working_tree")}>
                {t("Working tree", "工作树")}
              </button>
              <button type="button" className={diffMode === "branch" ? "active" : ""} onClick={() => setDiffMode("branch")}>
                {t("Against baseline", "相对基线")}
              </button>
              {dirtyTotal > 0 && (
                <button
                  type="button"
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
                </button>
              )}
            </div>

            <ChangeSection
              title={t(`Staged ${staged.length}`, `已暂存 ${staged.length}`)}
              entries={staged}
              selectedPath={selectedPath}
              busy={busy}
              onSelect={setSelectedPath}
              onStageAll={() => void runOp("stage_all")}
              onUnstageAll={() => void runOp("unstage_all")}
              onStage={(path) => void runOp("stage", { path })}
              onUnstage={(path) => void runOp("unstage", { path })}
              onIgnore={(path) => void runOp("add_to_gitignore", { path })}
              onDiscard={(entry) =>
                void runOp("discard", {
                  path: entry.path,
                  old_path: entry.old_path ?? undefined,
                  confirmTitle: t("Discard working tree changes", "撤销工作区修改"),
                  confirmDescription: t(`Restore ${entry.path}. Unsaved changes cannot be recovered.`, `将恢复 ${entry.path}，未保存修改无法恢复。`)
                })
              }
              section="staged"
            />
            <ChangeSection
              title={t(`Changes ${changes.length}`, `更改 ${changes.length}`)}
              entries={changes}
              selectedPath={selectedPath}
              busy={busy}
              onSelect={setSelectedPath}
              onStageAll={() => void runOp("stage_all")}
              onUnstageAll={() => void runOp("unstage_all")}
              onStage={(path) => void runOp("stage", { path })}
              onUnstage={(path) => void runOp("unstage", { path })}
              onIgnore={(path) => void runOp("add_to_gitignore", { path })}
              onDiscard={(entry) =>
                void runOp("discard", {
                  path: entry.path,
                  old_path: entry.old_path ?? undefined,
                  confirmTitle: entry.untracked ? t("Delete untracked file", "删除未跟踪文件") : t("Discard working tree changes", "撤销工作区修改"),
                  confirmDescription: entry.untracked
                    ? t(`Permanently delete “${entry.path}”.`, `将永久删除“${entry.path}”。`)
                    : t(`Restore ${entry.path}. Unsaved changes cannot be recovered.`, `将恢复 ${entry.path}，未保存修改无法恢复。`)
                })
              }
              section="changes"
            />

            <div className="git-remote-box">
              <span>{state?.remote_url ? t("Remote origin", "远端 origin") : t("Set origin remote", "设置 origin 远端")}</span>
              <input
                value={remoteUrl}
                onChange={(event) => setRemoteUrl(event.target.value)}
                placeholder="git@github.com:org/repo.git"
                spellCheck={false}
              />
              <button
                type="button"
                disabled={!remoteUrl.trim() || busy}
                onClick={() => void runOp("set_remote", { remote_url: remoteUrl })}
              >
                {state?.remote_url ? t("Update remote", "更新远端") : t("Save remote", "保存远端")}
              </button>
            </div>
          </section>

          <div className="diff-scroll">
            {reviewDiff.isLoading && <div className="git-clean diff-clean">{t("Loading diff...", "正在读取差异…")}</div>}
            {reviewDiff.error && <div className="pane-error">{reviewDiff.error.message}</div>}
            {reviewDiff.data?.patch ? (
              <div className="git-diff-shell">
                <div className="git-diff-meta">
                  {reviewDiff.data.base_ref} → {reviewDiff.data.head_ref}
                  {selectedPath ? ` · ${selectedPath}` : ""}
                </div>
                {reviewDiff.data.stat && <pre className="git-diff-stat">{reviewDiff.data.stat}</pre>}
                <DiffView source={reviewDiff.data.patch} headerPath={selectedPath ?? undefined} />
                {reviewDiff.data.truncated && <div className="git-clean">{t("Diff truncated", "差异已截断")}</div>}
              </div>
            ) : (
              !reviewDiff.isLoading && !reviewDiff.error && <div className="git-clean diff-clean">{t("No diff to display", "没有可显示的差异")}</div>
            )}
          </div>
        </div>
      ) : (
        <div className="git-manager-body">
          <section className="git-history-panel">
            <div className="git-change-head">
              <span>{t(`History ${commits.length}`, `历史 ${commits.length}`)}</span>
              {(history.data?.history_ahead || history.data?.history_behind) ? (
                <small className="git-history-sync">
                  <span><ArrowUp size={10} />{history.data?.history_ahead ?? 0}</span>
                  <span><ArrowDown size={10} />{history.data?.history_behind ?? 0}</span>
                </small>
              ) : null}
            </div>
            <div className="git-file-list">
              {commits.map((commit: GitCommitSummary) => (
                <button
                  type="button"
                  key={commit.sha}
                  className={`git-history-row${activeCommit === commit.sha ? " active" : ""}`}
                  onClick={() => {
                    setSelectedCommit(commit.sha);
                    setSelectedCommitPath(null);
                  }}
                >
                  <GitCommitHorizontal size={13} />
                  <span>
                    <strong>{commit.subject || commit.short_sha}</strong>
                    <small>
                      {commit.short_sha} · {commit.author_name} · {formatDate(commit.author_date, locale)}
                    </small>
                  </span>
                  {commit.local_only && <em>{t("local", "本地")}</em>}
                </button>
              ))}
              {commits.length === 0 && <div className="git-clean">{t("No commits yet", "暂无提交记录")}</div>}
              {commits.length >= historyLimit && (
                <button type="button" className="git-load-more" onClick={() => setHistoryLimit((value) => value + 40)}>
                  {t("Load more", "加载更多")}
                </button>
              )}
            </div>
          </section>
          <div className="diff-scroll">
            {activeCommit && commitDetails.data ? (
              <div className="git-diff-shell">
                <div className="git-commit-meta">
                  <h3>{commitDetails.data.commit.subject}</h3>
                  <p>
                    {commitDetails.data.commit.short_sha} · {commitDetails.data.commit.author_name} ·{" "}
                    {formatDate(commitDetails.data.commit.author_date, locale)}
                  </p>
                  {commitDetails.data.commit.body && <pre>{commitDetails.data.commit.body}</pre>}
                  <div className="git-commit-files">
                    {commitDetails.data.commit.files.map((file) => (
                      <button
                        type="button"
                        key={`${file.status}:${file.path}`}
                        className={selectedCommitPath === file.path ? "active" : ""}
                        onClick={() => setSelectedCommitPath(file.path)}
                      >
                        <span>{file.status}</span>
                        <strong>{file.path}</strong>
                      </button>
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
      )}

      {(error || notice || status.error) && (
        <div className={error || status.error ? "pane-error" : "pane-notice"}>
          {error?.message || status.error?.message || localizeApiMessage(notice, locale)}
        </div>
      )}
    </section>
  );
}

function ChangeSection(props: {
  title: string;
  entries: GitStatusEntry[];
  selectedPath: string | null;
  busy: boolean;
  section: "staged" | "changes";
  onSelect: (path: string) => void;
  onStageAll: () => void;
  onUnstageAll: () => void;
  onStage: (path: string) => void;
  onUnstage: (path: string) => void;
  onIgnore: (path: string) => void;
  onDiscard: (entry: GitStatusEntry) => void;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(true);
  return (
    <div className="git-section">
      <div className="git-change-head">
        <button type="button" className="git-section-toggle" onClick={() => setOpen((value) => !value)}>
          <ChevronDown size={12} className={open ? "open" : ""} />
          <span>{props.title}</span>
        </button>
        <span>
          {props.section === "staged" ? (
            <button type="button" onClick={props.onUnstageAll} title={t("Unstage all", "取消全部暂存")} disabled={props.busy}>
              <Minus size={12} />
            </button>
          ) : (
            <button type="button" onClick={props.onStageAll} title={t("Stage all", "暂存全部")} disabled={props.busy}>
              <Plus size={12} />
            </button>
          )}
        </span>
      </div>
      {open && (
        <div className="git-file-list">
          {props.entries.map((entry) => (
            <div
              className={`git-file-row${props.selectedPath === entry.path ? " active" : ""}`}
              key={`${entry.index_status}${entry.worktree_status}${entry.path}`}
            >
              <button type="button" className="git-file-main" onClick={() => props.onSelect(entry.path)}>
                <span className={`git-file-status tone-${statusTone(entry)}`}>{statusLabel(entry)}</span>
                <span title={entry.path}>{entry.path}</span>
              </button>
              <span className="git-file-actions">
                {props.section === "staged" && entry.staged && (
                  <button type="button" disabled={props.busy} onClick={() => props.onUnstage(entry.path)} title={t("Unstage", "取消暂存")}>
                    <Minus size={12} />
                  </button>
                )}
                {props.section === "changes" && (entry.untracked || entry.worktree_status !== "." || entry.conflicted) && (
                  <button type="button" disabled={props.busy} onClick={() => props.onStage(entry.path)} title={t("Stage", "暂存")}>
                    <Plus size={12} />
                  </button>
                )}
                {props.section === "changes" && entry.untracked && (
                  <button type="button" disabled={props.busy} onClick={() => props.onIgnore(entry.path)} title={t("Add to .gitignore", "加入 .gitignore")}>
                    <EyeOff size={12} />
                  </button>
                )}
                {(entry.untracked || entry.worktree_status !== ".") && (
                  <button
                    type="button"
                    disabled={props.busy}
                    onClick={() => props.onDiscard(entry)}
                    title={entry.untracked ? t("Delete untracked file", "删除未跟踪文件") : t("Discard changes", "撤销修改")}
                  >
                    {entry.untracked ? <Trash2 size={12} /> : <RotateCcw size={12} />}
                  </button>
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

function statusLabel(entry: GitStatusEntry) {
  if (entry.conflicted) return "U";
  if (entry.untracked) return "U";
  if (entry.staged && entry.worktree_status !== ".") return "M*";
  if (entry.staged) return entry.index_status === "A" ? "A" : entry.index_status === "D" ? "D" : "M";
  if (entry.worktree_status === "D") return "D";
  return "M";
}

function statusTone(entry: GitStatusEntry) {
  if (entry.conflicted) return "conflict";
  if (entry.untracked) return "untracked";
  if (entry.worktree_status === "D" || entry.index_status === "D") return "deleted";
  if (entry.index_status === "A") return "added";
  return "modified";
}

function formatDate(value: string, locale: string) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(locale);
}
