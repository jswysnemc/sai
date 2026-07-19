import type { GitConfig, GitStatusEntry } from "../../../api/contracts";

export type GitUntrackedChangesMode = GitConfig["untracked_changes"];

export type GitChangeGroups = {
  conflicts: GitStatusEntry[];
  staged: GitStatusEntry[];
  changes: GitStatusEntry[];
  untracked: GitStatusEntry[];
};

/**
 * 按 VS Code Source Control 的分区规则整理文件状态。
 *
 * @param entries Git porcelain 状态条目
 * @param untrackedMode 未跟踪文件显示方式
 * @returns 冲突、已暂存、工作树修改和未跟踪分区
 */
export function groupGitChanges(
  entries: GitStatusEntry[],
  untrackedMode: GitUntrackedChangesMode = "separate"
): GitChangeGroups {
  const untracked = entries.filter((entry) => entry.untracked);
  return {
    conflicts: entries.filter((entry) => entry.conflicted),
    staged: entries.filter((entry) => entry.staged && !entry.conflicted && !entry.untracked),
    changes: entries.filter(
      (entry) => !entry.conflicted && (
        entry.untracked ? untrackedMode === "mixed" : entry.worktree_status !== "."
      )
    ),
    untracked: untrackedMode === "separate" ? untracked : []
  };
}

/**
 * 计算当前显示策略下可见的唯一文件数量。
 *
 * @param entries Git porcelain 状态条目
 * @param untrackedMode 未跟踪文件显示方式
 * @returns 可见文件数量
 */
export function countVisibleGitChanges(
  entries: GitStatusEntry[],
  untrackedMode: GitUntrackedChangesMode
): number {
  return entries.filter((entry) => untrackedMode !== "hidden" || !entry.untracked).length;
}
