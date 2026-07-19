export const GIT_OPERATION_ACTIONS = [
  "init", "stage", "stage_all", "unstage", "unstage_all", "discard", "discard_all",
  "stage_patch", "unstage_patch", "discard_patch", "commit", "fetch", "pull",
  "pull_rebase", "push", "push_to", "force_push_with_lease", "sync", "set_remote", "publish",
  "switch_branch", "create_branch", "rename_branch", "delete_branch", "merge_branch",
  "rebase_branch", "checkout_commit", "cherry_pick", "rebase_onto", "reset_commit",
  "revert_commit", "add_to_gitignore", "stash_push", "stash_apply", "stash_pop",
  "stash_drop", "tag_create", "tag_delete", "remote_add", "remote_remove", "worktree_add",
  "worktree_remove", "resolve_conflict", "continue_operation", "skip_operation",
  "abort_operation"
] as const;

export type GitOperationAction = typeof GIT_OPERATION_ACTIONS[number];

export type GitOperationOptions = {
  repo_root?: string;
  path?: string;
  paths?: string[];
  old_path?: string;
  message?: string;
  remote_url?: string;
  branch?: string;
  branch_kind?: "local" | "remote";
  new_branch?: string;
  start_point?: string;
  post_action?: "push" | "sync";
  patch?: string;
  commit?: string;
  reset_mode?: "soft" | "mixed" | "hard";
  stash_ref?: string;
  tag?: string;
  remote_name?: string;
  worktree_path?: string;
  include_untracked?: boolean;
  exclude_untracked?: boolean;
  resolution?: "ours" | "theirs" | "content";
  content?: string;
  all?: boolean;
  amend?: boolean;
  signoff?: boolean;
  allow_empty?: boolean;
  force?: boolean;
};
