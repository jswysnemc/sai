import { ArrowDown, ArrowUp, FolderGit2, FolderOpen, GitBranch, RefreshCw, RotateCcw, Trash2, X } from "lucide-react";
import { useState } from "react";
import { api } from "../../../api/client";
import type { GitRepositoriesResponse, GitWorktree } from "../../../api/contracts";
import { Button } from "../../../shared/ui/button/button";
import { useConfirm } from "../../../shared/ui/dialog/dialog-provider";
import { useI18n } from "../../i18n/use-i18n";
import { switchWithTerminalConfirm } from "../../workspaces/workspace-switcher";
import type { RunGitOperation } from "../types";
import { WorktreeControls } from "./worktree-controls";
import "./repositories.css";

type RepositoriesViewProps = {
  data?: GitRepositoriesResponse;
  loading: boolean;
  error: Error | null;
  busy: boolean;
  selectedRoot: string | null;
  hiddenCount: number;
  onSelect: (root: string) => void;
  onClose: (root: string) => void;
  onShowAll: () => void;
  onRefresh: () => void;
  runOperation: RunGitOperation;
};

/**
 * 渲染工作区仓库、分支同步状态和关联 worktree。
 *
 * @param props 仓库数据、选择状态与操作回调
 * @returns Source Control Repositories 子视图
 */
export function RepositoriesView(props: RepositoriesViewProps) {
  const { t } = useI18n();
  const confirm = useConfirm();
  const [openError, setOpenError] = useState("");

  /**
   * 将 worktree 登记为工作区并切换到该目录。
   *
   * @param path worktree 绝对路径
   * @returns 无返回值
   */
  const openWorktree = async (path: string) => {
    try {
      setOpenError("");
      const workspace = await api.workspaces.add(path);
      const switched = await switchWithTerminalConfirm(workspace.id, confirm, t);
      if (switched) window.location.reload();
    } catch (error) {
      setOpenError(error instanceof Error ? error.message : String(error));
    }
  };

  /**
   * 经确认后移除关联 worktree。
   *
   * @param repositoryRoot worktree 所属仓库根目录
   * @param worktree 待移除 worktree
   * @returns 无返回值
   */
  const removeWorktree = async (repositoryRoot: string, worktree: GitWorktree) => {
    await props.runOperation("worktree_remove", {
      repo_root: repositoryRoot,
      worktree_path: worktree.path,
      confirmTitle: t("Remove worktree?", "移除 worktree？"),
      confirmDescription: t(
        `Remove ${worktree.path}. Git will refuse when it contains protected changes.`,
        `将移除 ${worktree.path}。存在受保护改动时 Git 会拒绝执行。`
      )
    });
  };

  if (props.loading && !props.data) {
    return <div className="git-repository-state">{t("Detecting repositories...", "正在检测仓库…")}</div>;
  }
  if (props.error) return <div className="git-repository-state error">{props.error.message}</div>;

  return (
    <section className="git-repositories-view">
      <header>
        <span><FolderGit2 size={14} /><strong>{t("Source Control Repositories", "源代码管理仓库")}</strong></span>
        <div>
          {props.hiddenCount > 0 && (
            <Button className="git-toolbar-icon" onClick={props.onShowAll} aria-label={t("Show closed repositories", "显示已关闭仓库")}>
              <RotateCcw size={13} />
            </Button>
          )}
          <Button className="git-toolbar-icon" onClick={props.onRefresh} aria-label={t("Refresh repositories", "刷新仓库")}>
            <RefreshCw size={13} />
          </Button>
        </div>
      </header>
      <WorktreeControls busy={props.busy} repositoryRoot={props.selectedRoot} runOperation={props.runOperation} />
      <div className="git-repository-list">
        {(props.data?.repositories ?? []).map((repository) => (
          <section className="git-repository-group" key={repository.root}>
            <div className="git-repository-heading">
              <Button
                className={`git-repository-item${props.selectedRoot === repository.root ? " active" : ""}`}
                onClick={() => props.onSelect(repository.root)}
              >
                <FolderGit2 size={14} />
                <span><strong>{repository.name}</strong><small>{repository.root}</small></span>
                <RepositoryMeta head={repository.head} changed={repository.changed} ahead={repository.ahead} behind={repository.behind} />
              </Button>
              <Button title={t("Close repository", "关闭仓库")} onClick={() => props.onClose(repository.root)}>
                <X size={12} />
              </Button>
            </div>
            {repository.error && <div className="git-repository-error">{repository.error}</div>}
            <div className="git-worktree-list">
              {repository.worktrees.filter((worktree) => !worktree.current).map((worktree) => (
                <div className={`git-worktree-item${props.selectedRoot === worktree.path ? " active" : ""}`} key={worktree.path}>
                  <Button onClick={() => props.onSelect(worktree.path)}>
                    <GitBranch size={12} />
                    <span><strong>{worktree.branch || t("Detached HEAD", "分离头指针")}</strong><small>{worktree.path}</small></span>
                    {worktree.locked && <em>{t("Locked", "已锁定")}</em>}
                  </Button>
                  <Button title={t("Open as workspace", "作为工作区打开")} onClick={() => void openWorktree(worktree.path)}>
                    <FolderOpen size={12} />
                  </Button>
                  {!worktree.current && (
                    <Button disabled={props.busy} title={t("Remove worktree", "移除 worktree")} onClick={() => void removeWorktree(repository.root, worktree)}>
                      <Trash2 size={12} />
                    </Button>
                  )}
                </div>
              ))}
            </div>
          </section>
        ))}
        {(props.data?.repositories.length ?? 0) === 0 && (
          <div className="git-repository-state">
            {props.hiddenCount > 0 ? t("All repositories are closed", "所有仓库均已关闭") : t("No Git repositories detected", "未检测到 Git 仓库")}
          </div>
        )}
      </div>
      {openError && <div className="git-repository-state error">{openError}</div>}
    </section>
  );
}

/**
 * 渲染仓库分支、改动数和远端同步计数。
 *
 * @param props 分支和计数数据
 * @returns 仓库摘要
 */
function RepositoryMeta(props: { head: string; changed: number; ahead: number; behind: number }) {
  return (
    <span className="git-repository-meta">
      <small>{props.head || "HEAD"}</small>
      {props.changed > 0 && <small>{props.changed}</small>}
      {props.ahead > 0 && <small><ArrowUp size={10} />{props.ahead}</small>}
      {props.behind > 0 && <small><ArrowDown size={10} />{props.behind}</small>}
    </span>
  );
}
