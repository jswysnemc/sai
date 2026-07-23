export type Workspace = {
  id: string;
  name: string;
  path: string;
  last_opened_at: string;
};

export type WorkspaceList = {
  active_id: string;
  workspaces: Workspace[];
};

export type DirectoryEntry = {
  name: string;
  path: string;
  git_repository: boolean;
};

export type DirectoryListing = {
  current: string;
  parent?: string | null;
  roots: DirectoryEntry[];
  entries: DirectoryEntry[];
};

export type FileNode = {
  name: string;
  path: string;
  kind: "file" | "directory" | "symlink";
  children: FileNode[];
};

export type FileContent = {
  path: string;
  content: string;
  size: number;
  modified_at?: number | null;
  version: string;
};

export type GitDiff = {
  repository: boolean;
  branch: string;
  status: string;
  files: GitFileStatus[];
  diff: string;
};

export type GitFileStatus = {
  path: string;
  index_status: string;
  worktree_status: string;
};

export type GitDirtyCounts = {
  staged: number;
  unstaged: number;
  untracked: number;
  conflicted: number;
};

export type GitStatusEntry = {
  path: string;
  old_path?: string | null;
  index_status: string;
  worktree_status: string;
  kind: string;
  staged: boolean;
  conflicted: boolean;
  untracked: boolean;
};

export type GitInProgressOperation = {
  kind: "merge" | "rebase" | "cherry_pick" | "revert" | string;
  can_continue: boolean;
  can_skip: boolean;
  can_abort: boolean;
};

export type GitRepositoryState = {
  repo_root: string;
  workdir: string;
  head: string;
  has_commits: boolean;
  upstream: string;
  remote_name: string;
  remote_url: string;
  ahead: number;
  behind: number;
  stash_count: number;
  dirty_counts: GitDirtyCounts;
  entries: GitStatusEntry[];
  operation: GitInProgressOperation | null;
  status: string;
  error?: string | null;
};

export type GitWorktree = {
  path: string;
  head: string;
  branch: string;
  bare: boolean;
  detached: boolean;
  locked: boolean;
  prunable: boolean;
  current: boolean;
};

export type GitRepositorySummary = {
  root: string;
  name: string;
  head: string;
  ahead: number;
  behind: number;
  changed: number;
  status: string;
  error?: string | null;
  worktrees: GitWorktree[];
};

export type GitRepositoriesResponse = {
  workspace_root: string;
  repositories: GitRepositorySummary[];
};

export type GitRepositoryStatusesResponse = {
  repositories: GitRepositoryState[];
};

export type GitBranch = {
  name: string;
  full_name: string;
  kind: string;
  current: boolean;
  upstream: string;
  ahead: number;
  behind: number;
};

export type GitBranchesResponse = {
  state: GitRepositoryState;
  branches: GitBranch[];
};

export type GitDiffResponse = {
  base_ref: string;
  head_ref: string;
  mode: string;
  files: string[];
  patch: string;
  stat: string;
  truncated: boolean;
  binary_files: string[];
};

export type GitCommitFile = {
  path: string;
  old_path?: string | null;
  status: string;
  kind: string;
};

export type GitCommitSummary = {
  sha: string;
  short_sha: string;
  parents: string[];
  refs: string[];
  subject: string;
  author_name: string;
  author_email: string;
  author_date: string;
  files: GitCommitFile[];
  file_count: number;
  local_only: boolean;
  remote_only: boolean;
};

export type GitLogResponse = {
  state: GitRepositoryState;
  commits: GitCommitSummary[];
  history_base_ref: string;
  history_remote_ref: string;
  history_ahead: number;
  history_behind: number;
  merge_base: string;
};

export type GitCommitDetails = {
  sha: string;
  short_sha: string;
  subject: string;
  body: string;
  author_name: string;
  author_email: string;
  author_date: string;
  files: GitCommitFile[];
  file_count: number;
  files_changed: number;
  insertions: number;
  deletions: number;
  stat: string;
  remote_name: string;
  remote_url: string;
};

export type GitCommitDetailsResponse = {
  state: GitRepositoryState;
  commit: GitCommitDetails;
};

export type GitOperationResponse = {
  ok: boolean;
  state: GitRepositoryState;
  stdout: string;
  stderr: string;
  message: string;
};

export type GitStashEntry = {
  reference: string;
  sha: string;
  subject: string;
  created_at: string;
};

export type GitTag = {
  name: string;
  sha: string;
  created_at: string;
  subject: string;
};

export type GitRemote = {
  name: string;
  fetch_url: string;
  push_url: string;
};

export type GitRepositoryResources = {
  state: GitRepositoryState;
  stashes: GitStashEntry[];
  tags: GitTag[];
  remotes: GitRemote[];
};

export type GitConflictContent = {
  state: GitRepositoryState;
  path: string;
  base: string | null;
  ours: string | null;
  theirs: string | null;
  current: string;
};

export type FileMutation = {
  path: string;
  kind: "file" | "directory";
};

export type PromptKind = "personas" | "identities";

export type PromptSummary = {
  name: string;
};

export type PromptDocument = PromptSummary & {
  content: string;
};
