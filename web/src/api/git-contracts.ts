export type GitOperationOptions = {
  path?: string;
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
  all?: boolean;
  amend?: boolean;
  signoff?: boolean;
  force?: boolean;
};
