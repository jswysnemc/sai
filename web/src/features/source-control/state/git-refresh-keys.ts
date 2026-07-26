import type { GitOperationAction } from "../../../api/git-contracts";

/**
 * 缓存失效的作用域。
 *
 * 一次 Git 操作往往只影响其中一两类数据，逐个作用域失效比整体重取省下大量请求：
 * 例如暂存单个文件不需要重新拉取提交历史和分支列表。
 */
export type GitRefreshScope = "status" | "history" | "branches" | "resources" | "workspace" | "repositories";

/** 各作用域覆盖的查询键前缀。 */
const SCOPE_QUERY_KEYS: Record<GitRefreshScope, string[]> = {
  status: ["git-status", "git-statuses", "git-review-diff", "git-conflict"],
  history: ["git-log", "git-commit-details", "git-commit-diff"],
  branches: ["git-branches"],
  resources: ["git-resources"],
  workspace: ["workspace-diff", "file-tree"],
  repositories: ["git-repositories"],
};

/** 全部作用域，用于无法归类的操作。 */
const ALL_SCOPES = Object.keys(SCOPE_QUERY_KEYS) as GitRefreshScope[];

/** 只改动工作区文件状态的操作。 */
const WORKING_TREE_SCOPES: GitRefreshScope[] = ["status", "workspace"];

/** 改动提交历史与工作区的操作。 */
const COMMIT_SCOPES: GitRefreshScope[] = ["status", "history", "branches", "workspace"];

/** 与远端同步的操作。 */
const REMOTE_SCOPES: GitRefreshScope[] = ["status", "history", "branches", "resources", "workspace"];

/** 操作到失效作用域的映射，未列出的操作按全量处理。 */
const ACTION_SCOPES: Partial<Record<GitOperationAction, GitRefreshScope[]>> = {
  stage: WORKING_TREE_SCOPES,
  stage_all: WORKING_TREE_SCOPES,
  unstage: WORKING_TREE_SCOPES,
  unstage_all: WORKING_TREE_SCOPES,
  discard: WORKING_TREE_SCOPES,
  discard_all: WORKING_TREE_SCOPES,
  stage_patch: WORKING_TREE_SCOPES,
  unstage_patch: WORKING_TREE_SCOPES,
  discard_patch: WORKING_TREE_SCOPES,
  add_to_gitignore: WORKING_TREE_SCOPES,
  resolve_conflict: WORKING_TREE_SCOPES,
  commit: COMMIT_SCOPES,
  revert_commit: COMMIT_SCOPES,
  cherry_pick: COMMIT_SCOPES,
  reset_commit: COMMIT_SCOPES,
  fetch: ["status", "history", "branches", "resources"],
  pull: REMOTE_SCOPES,
  pull_rebase: REMOTE_SCOPES,
  push: ["status", "history", "branches"],
  push_to: ["status", "history", "branches"],
  force_push_with_lease: ["status", "history", "branches"],
  sync: REMOTE_SCOPES,
  stash_push: ["status", "resources", "workspace"],
  stash_apply: ["status", "resources", "workspace"],
  stash_pop: ["status", "resources", "workspace"],
  stash_drop: ["resources"],
  tag_create: ["resources"],
  tag_delete: ["resources"],
  remote_add: ["resources", "branches"],
  remote_remove: ["resources", "branches"],
};

/**
 * 取一次操作需要失效的查询键前缀。
 *
 * @param action Git 操作标识
 * @returns 需要失效的查询键前缀；无法归类时返回全部
 */
export function refreshKeysFor(action: GitOperationAction | string): string[] {
  const scopes = ACTION_SCOPES[action as GitOperationAction] ?? ALL_SCOPES;
  return scopes.flatMap((scope) => SCOPE_QUERY_KEYS[scope]);
}

/**
 * 取全量刷新需要失效的查询键前缀。
 *
 * @returns 全部查询键前缀
 */
export function allRefreshKeys(): string[] {
  return ALL_SCOPES.flatMap((scope) => SCOPE_QUERY_KEYS[scope]);
}
