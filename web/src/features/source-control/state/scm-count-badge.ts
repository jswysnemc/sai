import type { GitRepositoryState, ScmConfig } from "../../../api/contracts";
import { countVisibleGitChanges, type GitUntrackedChangesMode } from "../changes/change-groups";

type RepositoryCountState = Pick<GitRepositoryState, "repo_root" | "entries">;

/**
 * 计算 Source Control 视图角标。
 *
 * @param mode 角标统计范围
 * @param repositories 当前可见仓库状态
 * @param focusedRoot 当前选中仓库根目录
 * @param untrackedMode 未跟踪文件显示方式
 * @returns 正数角标；关闭或无变更时返回空值
 */
export function resolveScmCountBadge(
  mode: ScmConfig["count_badge"],
  repositories: RepositoryCountState[],
  focusedRoot: string | null,
  untrackedMode: GitUntrackedChangesMode
): number | null {
  if (mode === "off") return null;
  const targets = mode === "focused"
    ? repositories.filter((repository) => repository.repo_root === focusedRoot)
    : repositories;
  const count = targets.reduce(
    (total, repository) => total + countVisibleGitChanges(repository.entries, untrackedMode),
    0
  );
  return count > 0 ? count : null;
}
