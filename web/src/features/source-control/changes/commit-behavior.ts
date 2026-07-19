import type { GitConfig } from "../../../api/contracts";
import type { GitOperationOptions } from "../../../api/git-contracts";

export type MainCommitKind = "staged" | "all" | "suggest_all" | "disabled";

/**
 * 根据暂存状态和 Smart Commit 设置选择主提交动作。
 *
 * @param stagedCount 已暂存文件数量
 * @param workingCount 可见工作区文件数量
 * @param enableSmartCommit 是否直接提交全部文件
 * @param suggestSmartCommit 是否在未暂存时提示提交全部文件
 * @returns 主提交动作类型
 */
export function resolveMainCommitKind(
  stagedCount: number,
  workingCount: number,
  enableSmartCommit: boolean,
  suggestSmartCommit: boolean
): MainCommitKind {
  if (stagedCount > 0) return "staged";
  if (workingCount === 0) return "disabled";
  if (enableSmartCommit) return "all";
  return suggestSmartCommit ? "suggest_all" : "disabled";
}

/**
 * 将 Git 设置应用到单个提交变体。
 *
 * @param options 提交变体原始选项
 * @param config Git 持久化配置
 * @returns 包含提交后动作和未跟踪文件策略的选项
 */
export function applyCommitConfig(
  options: GitOperationOptions,
  config: Pick<GitConfig, "post_commit_command" | "untracked_changes">
): GitOperationOptions {
  const postAction = options.post_action ?? (
    config.post_commit_command === "none" ? undefined : config.post_commit_command
  );
  return {
    ...options,
    post_action: postAction,
    exclude_untracked: options.all && config.untracked_changes === "hidden" ? true : undefined
  };
}
