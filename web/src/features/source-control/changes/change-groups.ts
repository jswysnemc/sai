import type { GitStatusEntry } from "../../../api/contracts";

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
 * @returns 冲突、已暂存、工作树修改和未跟踪分区
 */
export function groupGitChanges(entries: GitStatusEntry[]): GitChangeGroups {
  return {
    conflicts: entries.filter((entry) => entry.conflicted),
    staged: entries.filter((entry) => entry.staged && !entry.conflicted && !entry.untracked),
    changes: entries.filter(
      (entry) => !entry.conflicted && !entry.untracked && entry.worktree_status !== "."
    ),
    untracked: entries.filter((entry) => entry.untracked)
  };
}
