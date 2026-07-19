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
    pub operation: Option<GitInProgressOperation>,
    pub status: String,
    pub error: Option<String>,
}

/// 仓库中正在进行的 Git 操作。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitInProgressOperation {
    pub kind: String,
    pub can_continue: bool,
    pub can_skip: bool,
    pub can_abort: bool,
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
    pub remote_only: bool,
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
pub(crate) struct GitStashEntry {
    pub reference: String,
    pub sha: String,
    pub subject: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitTag {
    pub name: String,
    pub sha: String,
    pub created_at: String,
    pub subject: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitRemote {
    pub name: String,
    pub fetch_url: String,
    pub push_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitRepositoryResources {
    pub state: GitRepositoryState,
    pub stashes: Vec<GitStashEntry>,
    pub tags: Vec<GitTag>,
    pub remotes: Vec<GitRemote>,
}

/// Git worktree 摘要。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitWorktree {
    pub path: String,
    pub head: String,
    pub branch: String,
    pub bare: bool,
    pub detached: bool,
    pub locked: bool,
    pub prunable: bool,
    pub current: bool,
}

/// 工作区中单个 Git 仓库的轻量摘要。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitRepositorySummary {
    pub root: String,
    pub name: String,
    pub head: String,
    pub ahead: i32,
    pub behind: i32,
    pub changed: usize,
    pub status: String,
    pub error: Option<String>,
    pub worktrees: Vec<GitWorktree>,
}

/// 工作区仓库发现响应。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitRepositoriesResponse {
    pub workspace_root: String,
    pub repositories: Vec<GitRepositorySummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct GitConflictContent {
    pub state: GitRepositoryState,
    pub path: String,
    pub base: Option<String>,
    pub ours: Option<String>,
    pub theirs: Option<String>,
    pub current: String,
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

/// Git 操作的内部借用参数。
pub(crate) struct GitOperationRequest<'a> {
    pub(crate) action: &'a str,
    pub(crate) path: Option<&'a str>,
    pub(crate) old_path: Option<&'a str>,
    pub(crate) message: Option<&'a str>,
    pub(crate) remote_url: Option<&'a str>,
    pub(crate) branch: Option<&'a str>,
    pub(crate) branch_kind: Option<&'a str>,
    pub(crate) new_branch: Option<&'a str>,
    pub(crate) start_point: Option<&'a str>,
    pub(crate) post_action: Option<&'a str>,
    pub(crate) patch: Option<&'a str>,
    pub(crate) commit: Option<&'a str>,
    pub(crate) reset_mode: Option<&'a str>,
    pub(crate) stash_ref: Option<&'a str>,
    pub(crate) tag: Option<&'a str>,
    pub(crate) remote_name: Option<&'a str>,
    pub(crate) worktree_path: Option<&'a str>,
    pub(crate) workspace_root: Option<&'a str>,
    pub(crate) include_untracked: bool,
    pub(crate) resolution: Option<&'a str>,
    pub(crate) content: Option<&'a str>,
    pub(crate) all: bool,
    pub(crate) amend: bool,
    pub(crate) signoff: bool,
    pub(crate) force: bool,
}

impl<'a> GitOperationRequest<'a> {
    /// 创建仅包含操作名称的参数。
    ///
    /// 参数:
    /// - `action`: Git 操作名称
    ///
    /// 返回:
    /// - 空选项的操作参数
    pub(crate) fn new(action: &'a str) -> Self {
        Self {
            action,
            path: None,
            old_path: None,
            message: None,
            remote_url: None,
            branch: None,
            branch_kind: None,
            new_branch: None,
            start_point: None,
            post_action: None,
            patch: None,
            commit: None,
            reset_mode: None,
            stash_ref: None,
            tag: None,
            remote_name: None,
            worktree_path: None,
            workspace_root: None,
            include_untracked: false,
            resolution: None,
            content: None,
            all: false,
            amend: false,
            signoff: false,
            force: false,
        }
    }

    /// 附加仓库相对路径。
    ///
    /// 参数:
    /// - `path`: 可选路径
    ///
    /// 返回:
    /// - 更新后的操作参数
    pub(crate) fn with_path(mut self, path: &'a str) -> Self {
        self.path = Some(path);
        self
    }

    /// 附加可选提交说明。
    ///
    /// 参数:
    /// - `message`: 可选说明
    ///
    /// 返回:
    /// - 更新后的操作参数
    pub(crate) fn with_message(mut self, message: Option<&'a str>) -> Self {
        self.message = message;
        self
    }
}
