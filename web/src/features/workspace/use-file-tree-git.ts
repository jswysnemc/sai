import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { GitStatusEntry } from "../../api/contracts";
import type { GitOperationAction, GitOperationOptions } from "../../api/git-contracts";
import type { ChangeSectionKind } from "../source-control/changes/change-section";
import type { RunGitOperation } from "../source-control/types";
import { openWorkspaceDiff } from "./workspace-passive-diff";
import { workspaceRelativePath } from "./workspace-path-utils";

export type FileTreeGitEntry = {
  entry: GitStatusEntry;
  repoRoot: string;
};

/**
 * 为工作区文件树提供多仓库 Git 状态、操作和被动 Diff 打开能力。
 *
 * 返回:
 * - 按工作区路径索引的状态、操作函数、忙碌状态和错误
 */
export function useFileTreeGit() {
  const queryClient = useQueryClient();
  const [error, setError] = useState<Error | null>(null);
  const repositories = useQuery({
    queryKey: ["git-repositories"],
    queryFn: api.workspace.gitRepositories,
    retry: false,
    staleTime: 5_000
  });
  const roots = useMemo(
    () => repositories.data?.repositories.map((repository) => repository.root) ?? [],
    [repositories.data?.repositories]
  );
  const statuses = useQuery({
    queryKey: ["file-tree-git-statuses", roots],
    queryFn: () => api.workspace.gitStatuses(roots),
    enabled: roots.length > 0,
    retry: false,
    refetchInterval: 5_000
  });
  const entries = useMemo(() => {
    const result = new Map<string, FileTreeGitEntry>();
    const workspaceRoot = repositories.data?.workspace_root ?? "";
    for (const repository of statuses.data?.repositories ?? []) {
      for (const entry of repository.entries) {
        const absolute = `${repository.repo_root.replace(/[\\/]$/, "")}/${entry.path.replace(/^[\\/]/, "")}`;
        result.set(workspaceRelativePath(absolute, workspaceRoot), {
          entry,
          repoRoot: repository.repo_root
        });
      }
    }
    return result;
  }, [repositories.data?.workspace_root, statuses.data?.repositories]);

  const operation = useMutation({
    mutationFn: ({ action, options }: { action: GitOperationAction; options: GitOperationOptions }) =>
      api.workspace.gitOp(action, options),
    onSuccess: async (result) => {
      if (!result.ok) {
        setError(new Error(result.message || result.stderr || "Git operation failed"));
        return;
      }
      setError(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["file-tree-git-statuses"] }),
        queryClient.invalidateQueries({ queryKey: ["git-status"] }),
        queryClient.invalidateQueries({ queryKey: ["runtime-overview"] }),
        queryClient.invalidateQueries({ queryKey: ["workspace-diff"] })
      ]);
    },
    onError: (reason) => setError(toDisplayError(reason, "Git operation failed", "Git 操作失败"))
  });

  const runOperation: RunGitOperation = async (action, options = {}) => {
    try {
      return await operation.mutateAsync({ action, options });
    } catch {
      return undefined;
    }
  };

  /**
   * 读取指定文件差异并交给右侧栏显示。
   *
   * 参数:
   * - `item`: 文件 Git 状态和仓库根目录
   * - `workspacePath`: 文件树中的工作区相对路径
   * - `section`: 当前 Git 状态分区
   *
   * 返回:
   * - 请求完成后的 Promise
   */
  const openChanges = async (
    item: FileTreeGitEntry,
    workspacePath: string,
    section: ChangeSectionKind
  ) => {
    try {
      const mode = section === "staged" ? "staged" : section === "merge" ? "working_tree" : "unstaged";
      const response = await api.workspace.gitReviewDiff(mode, item.entry.path, item.repoRoot);
      openWorkspaceDiff({
        path: workspacePath,
        source: response.patch,
        title: workspacePath.split(/[\\/]/).filter(Boolean).at(-1) ?? workspacePath
      });
      setError(null);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to open diff", "打开差异失败"));
    }
  };

  /**
   * 比较同一仓库中的两个文件并在右侧栏显示。
   *
   * 参数:
   * - `base`: 作为基准的文件
   * - `head`: 当前文件
   * - `workspacePath`: 当前文件的工作区路径
   *
   * 返回:
   * - 请求完成后的 Promise
   */
  const compareFiles = async (
    base: FileTreeGitEntry,
    head: FileTreeGitEntry,
    workspacePath: string
  ) => {
    if (base.repoRoot !== head.repoRoot) {
      setError(new Error("Files from different repositories cannot be compared"));
      return;
    }
    try {
      const response = await api.workspace.gitFileDiff(base.entry.path, head.entry.path, head.repoRoot);
      openWorkspaceDiff({
        path: workspacePath,
        source: response.patch,
        title: workspacePath.split(/[\\/]/).filter(Boolean).at(-1) ?? workspacePath
      });
      setError(null);
    } catch (reason) {
      setError(toDisplayError(reason, "Failed to compare files", "比较文件失败"));
    }
  };

  return {
    entries,
    error,
    busy: operation.isPending,
    runOperation,
    openChanges,
    compareFiles
  };
}

/**
 * 返回文件状态所属的源代码管理分区。
 *
 * 参数:
 * - `entry`: Git 文件状态
 *
 * 返回:
 * - 变更分区
 */
export function fileTreeGitSection(entry: GitStatusEntry): ChangeSectionKind {
  if (entry.conflicted) return "merge";
  if (entry.untracked) return "untracked";
  if (entry.staged && entry.worktree_status === ".") return "staged";
  return "changes";
}

/**
 * 返回文件树使用的紧凑 Git 状态标签。
 *
 * 参数:
 * - `entry`: Git 文件状态
 *
 * 返回:
 * - 状态字母
 */
export function fileTreeGitStatusLabel(entry: GitStatusEntry): string {
  if (entry.conflicted) return "U";
  if (entry.untracked) return "?";
  if (entry.staged && entry.worktree_status !== ".") return "M*";
  if (entry.index_status === "A") return "A";
  if (entry.index_status === "D" || entry.worktree_status === "D") return "D";
  return "M";
}
