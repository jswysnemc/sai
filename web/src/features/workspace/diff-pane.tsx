import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Archive,
  Check,
  ChevronDown,
  CloudDownload,
  CloudUpload,
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
import type { GitBranch as GitBranchInfo, GitCommitSummary, GitStatusEntry } from "../../api/contracts";
import { useConfirm } from "../../shared/ui/dialog/dialog-provider";
import { DiffView } from "../chat/tool-renderers/diff-view";

type ReviewMode = "changes" | "history";
type DiffMode = "working_tree" | "branch";

/**
 * 渲染 LiveAgent 风格的 Git 变更与历史面板。
 *
 * @returns Git 管理面板
 */
export function DiffPane() {
  const confirm = useConfirm();
  const queryClient = useQueryClient();
  const [mode, setMode] = useState<ReviewMode>("changes");
  const [diffMode, setDiffMode] = useState<DiffMode>("working_tree");
  const [message, setMessage] = useState("");
  const [initBranch, setInitBranch] = useState("main");
  const [createBranchName, setCreateBranchName] = useState("");
  const [remoteUrl, setRemoteUrl] = useState("");
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [selectedCommit, setSelectedCommit] = useState<string | null>(null);
  const [selectedCommitPath, setSelectedCommitPath] = useState<string | null>(null);
  const [historyLimit, setHistoryLimit] = useState(40);
  const [branchMenuOpen, setBranchMenuOpen] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");

  const status = useQuery({
    queryKey: ["git-status"],
    queryFn: api.workspace.gitStatus,
    refetchInterval: 8_000
  });
  const branches = useQuery({
    queryKey: ["git-branches"],
    queryFn: api.workspace.gitBranches,
    enabled: status.data?.status === "ready",
    staleTime: 10_000
  });
  const history = useQuery({
    queryKey: ["git-log", historyLimit],
    queryFn: () => api.workspace.gitLog(historyLimit, 0),
    enabled: status.data?.status === "ready",
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
    enabled: Boolean(selectedCommit)
  });
  const commitDiff = useQuery({
    queryKey: ["git-commit-diff", selectedCommit, selectedCommitPath],
    queryFn: () => api.workspace.gitCommitDiff(selectedCommit!, selectedCommitPath ?? undefined),
    enabled: Boolean(selectedCommit)
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
  const localBranches = useMemo(
    () => (branches.data?.branches ?? []).filter((branch) => branch.kind === "local"),
    [branches.data?.branches]
  );
  const remoteBranches = useMemo(
    () => (branches.data?.branches ?? []).filter((branch) => branch.kind === "remote"),
    [branches.data?.branches]
  );

  useEffect(() => {
    if (state?.remote_url) setRemoteUrl(state.remote_url);
  }, [state?.remote_url]);

  useEffect(() => {
    if (!branchMenuOpen) return;
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target as HTMLElement | null;
      if (!target?.closest(".git-branch-menu") && !target?.closest(".git-branch-trigger")) {
        setBranchMenuOpen(false);
      }
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [branchMenuOpen]);

  const refreshAll = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: ["git-status"] }),
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
    mutationFn: (input: { action: string; path?: string; message?: string; remote_url?: string }) =>
      api.workspace.gitOp(input.action, {
        path: input.path,
        message: input.message,
        remote_url: input.remote_url
      }),
    onSuccess: async (result) => {
      if (!result.ok) {
        setError(result.message || result.stderr || "Git 操作失败");
        setNotice("");
        return;
      }
      setError("");
      setNotice(result.message);
      queryClient.setQueryData(["git-status"], result.state);
      await refreshAll();
    },
    onError: (reason) => {
      setError(reason instanceof Error ? reason.message : String(reason));
      setNotice("");
    }
  });

  const runOp = async (
    action: string,
    options: {
      path?: string;
      message?: string;
      remote_url?: string;
      confirmTitle?: string;
      confirmDescription?: string;
    } = {}
  ) => {
    if (options.confirmTitle) {
      const confirmed = await confirm({
        title: options.confirmTitle,
        description: options.confirmDescription ?? "此操作可能无法撤销。",
        confirmLabel: "继续",
        danger: true
      });
      if (!confirmed) return;
    }
    setError("");
    setNotice("");
    await op.mutateAsync({
      action,
      path: options.path,
      message: options.message,
      remote_url: options.remote_url
    });
    if (action === "commit") setMessage("");
    if (action === "create_branch") {
      setCreateBranchName("");
      setBranchMenuOpen(false);
    }
    if (action === "switch_branch") setBranchMenuOpen(false);
  };

  if (status.isLoading && !state) {
    return (
      <section className="diff-pane git-manager">
        <div className="git-clean">正在读取 Git 状态…</div>
      </section>
    );
  }

  if (!ready) {
    return (
      <section className="diff-pane git-manager">
        <header className="panel-head">
          <div>
            <span className="eyebrow">Git 工作区</span>
            <h2>
              <GitBranch size={15} />
              版本管理
            </h2>
          </div>
          <button type="button" className="icon-button" onClick={() => void status.refetch()} aria-label="刷新">
            <RefreshCw size={14} />
          </button>
        </header>
        <div className="git-init-panel">
          <GitBranch size={24} />
          <h3>初始化 Git 仓库</h3>
          <p>为当前工作区创建本地版本历史，并支持后续 fetch / pull / push。</p>
          <label>
            <span>默认分支</span>
            <input value={initBranch} onChange={(event) => setInitBranch(event.target.value)} spellCheck={false} />
          </label>
          <button
            type="button"
            onClick={() => void runOp("init", { message: initBranch })}
            disabled={!initBranch.trim() || op.isPending}
          >
            初始化仓库
          </button>
        </div>
        {(status.error || error) && <div className="pane-error">{error || status.error?.message}</div>}
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
        <div className="git-review-branch">
          <button
            type="button"
            className="git-branch-trigger"
            onClick={() => setBranchMenuOpen((value) => !value)}
            aria-expanded={branchMenuOpen}
          >
            <GitBranch size={14} />
            <strong title={state?.head}>{state?.head || "HEAD"}</strong>
            <ChevronDown size={12} className={branchMenuOpen ? "open" : ""} />
          </button>
          {(state?.ahead || state?.behind) ? (
            <span className="git-review-sync">
              {state.ahead > 0 && <b>↑{state.ahead}</b>}
              {state.behind > 0 && <i>↓{state.behind}</i>}
            </span>
          ) : null}
          {state?.upstream && <small title={state.upstream}>{state.upstream}</small>}
          {branchMenuOpen && (
            <div className="git-branch-menu">
              <div className="git-branch-create">
                <input
                  value={createBranchName}
                  onChange={(event) => setCreateBranchName(event.target.value)}
                  placeholder="新建分支名"
                  spellCheck={false}
                />
                <button
                  type="button"
                  disabled={!createBranchName.trim() || busy}
                  onClick={() => void runOp("create_branch", { message: createBranchName.trim() })}
                >
                  创建
                </button>
              </div>
              <BranchGroup
                title="本地分支"
                branches={localBranches}
                busy={busy}
                onSelect={(branch) => void runOp("switch_branch", { message: branch.full_name })}
              />
              <BranchGroup
                title="远程分支"
                branches={remoteBranches}
                busy={busy}
                onSelect={(branch) => void runOp("switch_branch", { message: branch.full_name })}
              />
            </div>
          )}
        </div>
        <div className="git-review-actions">
          <button type="button" className={mode === "changes" ? "active" : ""} onClick={() => setMode("changes")}>
            变更
          </button>
          <button type="button" className={mode === "history" ? "active" : ""} onClick={() => setMode("history")}>
            <History size={13} />
            历史
          </button>
          <button type="button" disabled={busy} onClick={() => void runOp("fetch")} title="Fetch">
            <CloudDownload size={13} />
          </button>
          <button type="button" disabled={busy} onClick={() => void runOp("pull")} title="Pull">
            <RefreshCw size={13} />
          </button>
          <button type="button" disabled={busy} onClick={() => void runOp("push")} title="Push">
            <CloudUpload size={13} />
          </button>
          <button
            type="button"
            disabled={busy || dirtyTotal === 0}
            onClick={() => void runOp("stash_push", { message: "Sai stash" })}
            title="Stash"
          >
            <Archive size={13} />
          </button>
          {(state?.stash_count ?? 0) > 0 && (
            <button type="button" disabled={busy} onClick={() => void runOp("stash_pop")} title={`弹出 stash (${state?.stash_count})`}>
              pop
            </button>
          )}
          <button type="button" disabled={busy} onClick={() => void refreshAll()} title="刷新" aria-label="刷新">
            <RefreshCw size={13} />
          </button>
        </div>
      </header>

      {mode === "changes" ? (
        <div className="git-manager-body">
          <section className="git-change-panel">
            <div className="git-commit-box">
              <textarea rows={3} value={message} onChange={(event) => setMessage(event.target.value)} placeholder="提交说明" />
              <button
                type="button"
                onClick={() => void runOp("commit", { message })}
                disabled={!message.trim() || busy || (state?.dirty_counts.staged ?? 0) === 0}
              >
                <Check size={13} />
                提交已暂存变更
              </button>
            </div>

            <div className="git-diff-mode">
              <button type="button" className={diffMode === "working_tree" ? "active" : ""} onClick={() => setDiffMode("working_tree")}>
                工作树
              </button>
              <button type="button" className={diffMode === "branch" ? "active" : ""} onClick={() => setDiffMode("branch")}>
                相对基线
              </button>
              {dirtyTotal > 0 && (
                <button
                  type="button"
                  className="danger"
                  disabled={busy}
                  onClick={() =>
                    void runOp("discard_all", {
                      confirmTitle: "丢弃全部改动",
                      confirmDescription: "将放弃所有已暂存、未暂存和未跟踪改动，此操作无法撤销。"
                    })
                  }
                >
                  全部丢弃
                </button>
              )}
            </div>

            <ChangeSection
              title={`已暂存 ${staged.length}`}
              entries={staged}
              selectedPath={selectedPath}
              busy={busy}
              onSelect={setSelectedPath}
              onStageAll={() => void runOp("stage_all")}
              onUnstageAll={() => void runOp("unstage_all")}
              onStage={(path) => void runOp("stage", { path })}
              onUnstage={(path) => void runOp("unstage", { path })}
              onDiscard={(path) =>
                void runOp("discard", {
                  path,
                  confirmTitle: "撤销工作区修改",
                  confirmDescription: `将恢复 ${path}，未保存修改无法恢复。`
                })
              }
              section="staged"
            />
            <ChangeSection
              title={`更改 ${changes.length}`}
              entries={changes}
              selectedPath={selectedPath}
              busy={busy}
              onSelect={setSelectedPath}
              onStageAll={() => void runOp("stage_all")}
              onUnstageAll={() => void runOp("unstage_all")}
              onStage={(path) => void runOp("stage", { path })}
              onUnstage={(path) => void runOp("unstage", { path })}
              onDiscard={(path) =>
                void runOp("discard", {
                  path,
                  confirmTitle: entryIsUntracked(changes, path) ? "删除未跟踪文件" : "撤销工作区修改",
                  confirmDescription: entryIsUntracked(changes, path)
                    ? `将永久删除“${path}”。`
                    : `将恢复 ${path}，未保存修改无法恢复。`
                })
              }
              section="changes"
            />

            <div className="git-remote-box">
              <span>{state?.remote_url ? "远端 origin" : "设置 origin 远端"}</span>
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
                {state?.remote_url ? "更新远端" : "保存远端"}
              </button>
            </div>
          </section>

          <div className="diff-scroll">
            {reviewDiff.isLoading && <div className="git-clean diff-clean">正在读取差异…</div>}
            {reviewDiff.error && <div className="pane-error">{reviewDiff.error.message}</div>}
            {reviewDiff.data?.patch ? (
              <div className="git-diff-shell">
                <div className="git-diff-meta">
                  {reviewDiff.data.base_ref} → {reviewDiff.data.head_ref}
                  {selectedPath ? ` · ${selectedPath}` : ""}
                </div>
                {reviewDiff.data.stat && <pre className="git-diff-stat">{reviewDiff.data.stat}</pre>}
                <DiffView source={reviewDiff.data.patch} headerPath={selectedPath ?? undefined} />
                {reviewDiff.data.truncated && <div className="git-clean">差异已截断</div>}
              </div>
            ) : (
              !reviewDiff.isLoading && !reviewDiff.error && <div className="git-clean diff-clean">没有可显示的差异</div>
            )}
          </div>
        </div>
      ) : (
        <div className="git-manager-body">
          <section className="git-history-panel">
            <div className="git-change-head">
              <span>历史 {commits.length}</span>
              {(history.data?.history_ahead || history.data?.history_behind) ? (
                <small>
                  ↑{history.data?.history_ahead ?? 0} ↓{history.data?.history_behind ?? 0}
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
                      {commit.short_sha} · {commit.author_name} · {formatDate(commit.author_date)}
                    </small>
                  </span>
                  {commit.local_only && <em>local</em>}
                </button>
              ))}
              {commits.length === 0 && <div className="git-clean">暂无提交记录</div>}
              {commits.length >= historyLimit && (
                <button type="button" className="git-load-more" onClick={() => setHistoryLimit((value) => value + 40)}>
                  加载更多
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
                    {formatDate(commitDetails.data.commit.author_date)}
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
                  <div className="git-clean">没有可显示的提交差异</div>
                )}
              </div>
            ) : (
              <div className="git-clean diff-clean">选择一条提交查看详情</div>
            )}
          </div>
        </div>
      )}

      {(error || notice || status.error) && (
        <div className={error || status.error ? "pane-error" : "pane-notice"}>
          {error || status.error?.message || notice}
        </div>
      )}
    </section>
  );
}

function BranchGroup(props: {
  title: string;
  branches: GitBranchInfo[];
  busy: boolean;
  onSelect: (branch: GitBranchInfo) => void;
}) {
  if (props.branches.length === 0) return null;
  return (
    <div className="git-branch-group">
      <span>{props.title}</span>
      {props.branches.map((branch) => (
        <button
          type="button"
          key={`${branch.kind}:${branch.full_name}`}
          className={branch.current ? "active" : ""}
          disabled={props.busy || branch.current}
          onClick={() => props.onSelect(branch)}
        >
          <strong>{branch.name}</strong>
          {branch.upstream && <small>{branch.upstream}</small>}
        </button>
      ))}
    </div>
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
  onDiscard: (path: string) => void;
}) {
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
            <button type="button" onClick={props.onUnstageAll} title="取消全部暂存" disabled={props.busy}>
              <Minus size={12} />
            </button>
          ) : (
            <button type="button" onClick={props.onStageAll} title="暂存全部" disabled={props.busy}>
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
                {entry.staged && (
                  <button type="button" disabled={props.busy} onClick={() => props.onUnstage(entry.path)} title="取消暂存">
                    <Minus size={12} />
                  </button>
                )}
                {(entry.untracked || entry.worktree_status !== "." || entry.conflicted) && !entry.staged && (
                  <button type="button" disabled={props.busy} onClick={() => props.onStage(entry.path)} title="暂存">
                    <Plus size={12} />
                  </button>
                )}
                {(entry.untracked || entry.worktree_status !== ".") && (
                  <button
                    type="button"
                    disabled={props.busy}
                    onClick={() => props.onDiscard(entry.path)}
                    title={entry.untracked ? "删除未跟踪文件" : "撤销修改"}
                  >
                    {entry.untracked ? <Trash2 size={12} /> : <RotateCcw size={12} />}
                  </button>
                )}
              </span>
            </div>
          ))}
          {props.entries.length === 0 && <div className="git-clean">无文件</div>}
        </div>
      )}
    </div>
  );
}

function entryIsUntracked(entries: GitStatusEntry[], path: string) {
  return entries.some((entry) => entry.path === path && entry.untracked);
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

function formatDate(value: string) {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
}
