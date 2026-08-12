import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { api } from "../../api/client";
import { toDisplayError } from "../../api/api-error";
import type { GitStatusEntry } from "../../api/contracts";
import type { GitOperationAction, GitOperationOptions } from "../../api/git-contracts";
import type { ChangeSectionKind } from "../source-control/changes/change-section";
import type { RunGitOperation } from "../source-control/types";
import { openWorkspaceDiff } from "./workspace-passive-diff";
import { useWorkspaceGitEntries, type FileTreeGitEntry } from "./use-workspace-git-entries";

export type { FileTreeGitEntry } from "./use-workspace-git-entries";

/**
 * 为工作区文件树提供多仓库 Git 状态、操作和被动 Diff 打开能力。
 *
 * 返回:
 * - 按工作区路径索引的状态、操作函数、忙碌状态和错误
 */
export function useFileTreeGit() {
  const queryClient = useQueryClient();
  const [error, setError] = useState<Error | null>(null);
  const { entries } = useWorkspaceGitEntries();

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
 * 返回文件树与编辑器标签共用的紧凑 Git 状态字母。
 *
 * 字母语义对齐 VS Code / Cursor：未跟踪为 U、冲突为 !，
 * 一眼即可与「修改 M / 新增 A / 删除 D」区分。
 *
 * 参数:
 * - `entry`: Git 文件状态
 *
 * 返回:
 * - 状态字母
 */
export function fileTreeGitStatusLabel(entry: GitStatusEntry): string {
  if (entry.conflicted) return "!";
  if (entry.untracked) return "U";
  if (entry.staged && entry.worktree_status !== ".") return "M*";
  if (entry.index_status === "A") return "A";
  if (entry.index_status === "D" || entry.worktree_status === "D") return "D";
  return "M";
}

/**
 * 将 Git 状态归类为文件名染色色调。
 *
 * 参数:
 * - `entry`: Git 状态条目
 *
 * 返回:
 * - added / deleted / conflicted / modified 之一
 */
export function fileTreeGitStatusTone(entry: GitStatusEntry): FileTreeGitTone {
  if (entry.conflicted) return "conflicted";
  if (entry.untracked || entry.index_status === "A") return "added";
  if (entry.index_status === "D" || entry.worktree_status === "D") return "deleted";
  return "modified";
}

export type FileTreeGitTone = "added" | "modified" | "deleted" | "conflicted";

/** 色调聚合优先级：目录取子项中最高优先级的色调 */
const TONE_PRIORITY: Record<FileTreeGitTone, number> = {
  conflicted: 3,
  added: 2,
  modified: 1,
  deleted: 0
};

/**
 * 把文件级 Git 状态向上冒泡为目录色调。
 *
 * 折叠的目录也能看出内部有变更：每个文件条目为其全部祖先目录
 * 记一笔，按优先级（冲突 > 新增 > 修改 > 删除）保留最高色调。
 *
 * 参数:
 * - `entries`: 按工作区相对路径索引的文件状态
 *
 * 返回:
 * - 目录路径到聚合色调的映射
 */
export function directoryGitTones(
  entries: ReadonlyMap<string, FileTreeGitEntry>
): Map<string, FileTreeGitTone> {
  const tones = new Map<string, FileTreeGitTone>();
  for (const [path, item] of entries) {
    const tone = fileTreeGitStatusTone(item.entry);
    // 1. 逐级向上累计祖先目录路径
    const segments = path.split("/");
    let prefix = "";
    for (let index = 0; index < segments.length - 1; index += 1) {
      prefix = prefix ? `${prefix}/${segments[index]}` : segments[index];
      const current = tones.get(prefix);
      if (!current || TONE_PRIORITY[tone] > TONE_PRIORITY[current]) {
        tones.set(prefix, tone);
      }
    }
  }
  return tones;
}
