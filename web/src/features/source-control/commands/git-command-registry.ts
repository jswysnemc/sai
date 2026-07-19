import type { GitOperationAction } from "../../../api/git-contracts";
import type { GitOperationUiOptions, RunGitOperation } from "../types";

export type GitCommandGroup = "commit" | "changes" | "remote" | "branch" | "stash" | "history" | "repository" | "operation";

export type GitCommandDefinition = {
  id: GitCommandId;
  action: GitOperationAction;
  group: GitCommandGroup;
  destructive: boolean;
};

export type GitCommandId =
  | "git.init"
  | "git.stage"
  | "git.stageAll"
  | "git.unstage"
  | "git.unstageAll"
  | "git.clean"
  | "git.cleanAll"
  | "git.stageSelectedRanges"
  | "git.unstageSelectedRanges"
  | "git.revertSelectedRanges"
  | "git.commit"
  | "git.fetch"
  | "git.pull"
  | "git.pullRebase"
  | "git.push"
  | "git.pushTo"
  | "git.pushForce"
  | "git.sync"
  | "git.setRemote"
  | "git.publish"
  | "git.checkout"
  | "git.branch"
  | "git.renameBranch"
  | "git.deleteBranch"
  | "git.merge"
  | "git.rebase"
  | "git.checkoutCommit"
  | "git.cherryPick"
  | "git.rebaseOnto"
  | "git.revertCommit"
  | "git.reset"
  | "git.addToGitignore"
  | "git.stash"
  | "git.stashApply"
  | "git.stashPop"
  | "git.stashDrop"
  | "git.createTag"
  | "git.deleteTag"
  | "git.addRemote"
  | "git.removeRemote"
  | "git.worktreeAdd"
  | "git.worktreeRemove"
  | "git.resolveConflict"
  | "git.continue"
  | "git.skip"
  | "git.abort";

const COMMANDS: GitCommandDefinition[] = [
  command("git.init", "init", "repository"),
  command("git.stage", "stage", "changes"),
  command("git.stageAll", "stage_all", "changes"),
  command("git.unstage", "unstage", "changes"),
  command("git.unstageAll", "unstage_all", "changes"),
  command("git.clean", "discard", "changes", true),
  command("git.cleanAll", "discard_all", "changes", true),
  command("git.stageSelectedRanges", "stage_patch", "changes"),
  command("git.unstageSelectedRanges", "unstage_patch", "changes"),
  command("git.revertSelectedRanges", "discard_patch", "changes", true),
  command("git.commit", "commit", "commit"),
  command("git.fetch", "fetch", "remote"),
  command("git.pull", "pull", "remote"),
  command("git.pullRebase", "pull_rebase", "remote"),
  command("git.push", "push", "remote"),
  command("git.pushTo", "push_to", "remote"),
  command("git.pushForce", "force_push_with_lease", "remote", true),
  command("git.sync", "sync", "remote"),
  command("git.setRemote", "set_remote", "remote"),
  command("git.publish", "publish", "remote"),
  command("git.checkout", "switch_branch", "branch"),
  command("git.branch", "create_branch", "branch"),
  command("git.renameBranch", "rename_branch", "branch"),
  command("git.deleteBranch", "delete_branch", "branch", true),
  command("git.merge", "merge_branch", "branch"),
  command("git.rebase", "rebase_branch", "branch"),
  command("git.checkoutCommit", "checkout_commit", "history"),
  command("git.cherryPick", "cherry_pick", "history"),
  command("git.rebaseOnto", "rebase_onto", "history"),
  command("git.revertCommit", "revert_commit", "history"),
  command("git.reset", "reset_commit", "history", true),
  command("git.addToGitignore", "add_to_gitignore", "changes"),
  command("git.stash", "stash_push", "stash"),
  command("git.stashApply", "stash_apply", "stash"),
  command("git.stashPop", "stash_pop", "stash"),
  command("git.stashDrop", "stash_drop", "stash", true),
  command("git.createTag", "tag_create", "repository"),
  command("git.deleteTag", "tag_delete", "repository", true),
  command("git.addRemote", "remote_add", "repository"),
  command("git.removeRemote", "remote_remove", "repository", true),
  command("git.worktreeAdd", "worktree_add", "repository"),
  command("git.worktreeRemove", "worktree_remove", "repository", true),
  command("git.resolveConflict", "resolve_conflict", "changes"),
  command("git.continue", "continue_operation", "operation"),
  command("git.skip", "skip_operation", "operation"),
  command("git.abort", "abort_operation", "operation", true)
];

export const GIT_COMMANDS: ReadonlyMap<GitCommandId, GitCommandDefinition> = new Map(
  COMMANDS.map((definition) => [definition.id, definition])
);

/**
 * 执行注册命令，并把命令参数原样交给 Git 操作层。
 *
 * @param id VS Code 风格命令标识
 * @param runOperation Git 操作执行器
 * @param options 操作参数和界面确认信息
 * @returns Git 操作结果
 */
export function executeGitCommand(
  id: GitCommandId,
  runOperation: RunGitOperation,
  options: GitOperationUiOptions = {}
) {
  const definition = GIT_COMMANDS.get(id);
  if (!definition) throw new Error(`Unknown Git command: ${id}`);
  return runOperation(definition.action, options);
}

/**
 * 创建单条不可变命令定义。
 *
 * @param id 命令标识
 * @param action 后端操作名称
 * @param group 菜单分组
 * @param destructive 是否属于破坏性操作
 * @returns 命令定义
 */
function command(
  id: GitCommandId,
  action: GitOperationAction,
  group: GitCommandGroup,
  destructive = false
): GitCommandDefinition {
  return { id, action, group, destructive };
}
