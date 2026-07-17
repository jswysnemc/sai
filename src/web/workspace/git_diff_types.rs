use serde::{Deserialize, Serialize};

/// 旧版兼容：单个 Git 文件状态。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct GitFileStatus {
    pub path: String,
    pub index_status: String,
    pub worktree_status: String,
}

/// 旧版兼容：当前工作区 Git 状态与 Diff。
#[derive(Clone, Debug, Serialize)]
pub(crate) struct GitDiff {
    pub repository: bool,
    pub branch: String,
    pub status: String,
    pub files: Vec<GitFileStatus>,
    pub diff: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct GitDirtyCounts {
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub conflicted: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitStatusEntry {
    pub path: String,
    pub old_path: Option<String>,
    pub index_status: String,
    pub worktree_status: String,
    pub kind: String,
    pub staged: bool,
    pub conflicted: bool,
    pub untracked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitRepositoryState {
    pub repo_root: String,
    pub workdir: String,
    pub head: String,
    pub upstream: String,
    pub remote_name: String,
    pub remote_url: String,
    pub ahead: i32,
    pub behind: i32,
    pub stash_count: i32,
    pub dirty_counts: GitDirtyCounts,
    pub entries: Vec<GitStatusEntry>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitBranch {
    pub name: String,
    pub full_name: String,
    pub kind: String,
    pub current: bool,
    pub upstream: String,
    pub ahead: i32,
    pub behind: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitBranchesResponse {
    pub state: GitRepositoryState,
    pub branches: Vec<GitBranch>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitDiffResponse {
    pub base_ref: String,
    pub head_ref: String,
    pub mode: String,
    pub files: Vec<String>,
    pub patch: String,
    pub stat: String,
    pub truncated: bool,
    pub binary_files: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitCommitFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitCommitSummary {
    pub sha: String,
    pub short_sha: String,
    pub parents: Vec<String>,
    pub refs: Vec<String>,
    pub subject: String,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub files: Vec<GitCommitFile>,
    pub file_count: usize,
    pub local_only: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitLogResponse {
    pub state: GitRepositoryState,
    pub commits: Vec<GitCommitSummary>,
    pub history_base_ref: String,
    pub history_remote_ref: String,
    pub history_ahead: i32,
    pub history_behind: i32,
    pub merge_base: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitCommitDetails {
    pub sha: String,
    pub short_sha: String,
    pub subject: String,
    pub body: String,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub files: Vec<GitCommitFile>,
    pub file_count: usize,
    pub files_changed: usize,
    pub insertions: usize,
    pub deletions: usize,
    pub stat: String,
    pub remote_name: String,
    pub remote_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitCommitDetailsResponse {
    pub state: GitRepositoryState,
    pub commit: GitCommitDetails,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitOperationResponse {
    pub ok: bool,
    pub state: GitRepositoryState,
    pub stdout: String,
    pub stderr: String,
    pub message: String,
}

pub(super) struct GitOutput {
    pub(super) stdout: String,
    pub(super) stderr: String,
}
