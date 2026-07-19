use serde::de::Error as DeError;
use serde::{Deserialize, Deserializer, Serialize};

const DEFAULT_WORKTREE_LIMIT: usize = 10;

/// Source Control 通用显示配置。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScmConfig {
    pub default_view_mode: String,
    pub count_badge: String,
}

/// Git 仓库发现、提交与远端操作配置。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct GitConfig {
    pub auto_repository_detection: bool,
    pub untracked_changes: String,
    pub enable_smart_commit: bool,
    pub suggest_smart_commit: bool,
    pub confirm_sync: bool,
    pub confirm_force_push: bool,
    pub confirm_empty_commits: bool,
    pub post_commit_command: String,
    pub show_action_button: bool,
    pub detect_worktrees: bool,
    pub detect_worktrees_limit: usize,
    pub autofetch: bool,
    pub branch_random_name: GitBranchRandomNameConfig,
}

/// 新建分支名称建议配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct GitBranchRandomNameConfig {
    #[serde(default)]
    pub enable: bool,
}

#[derive(Deserialize)]
struct RawScmConfig {
    #[serde(default = "default_view_mode")]
    default_view_mode: String,
    #[serde(default = "default_count_badge")]
    count_badge: String,
}

#[derive(Deserialize)]
struct RawGitConfig {
    #[serde(default = "default_true")]
    auto_repository_detection: bool,
    #[serde(default = "default_untracked_changes")]
    untracked_changes: String,
    #[serde(default)]
    enable_smart_commit: bool,
    #[serde(default = "default_true")]
    suggest_smart_commit: bool,
    #[serde(default = "default_true")]
    confirm_sync: bool,
    #[serde(default = "default_true")]
    confirm_force_push: bool,
    #[serde(default = "default_true")]
    confirm_empty_commits: bool,
    #[serde(default = "default_post_commit_command")]
    post_commit_command: String,
    #[serde(default = "default_true")]
    show_action_button: bool,
    #[serde(default = "default_true")]
    detect_worktrees: bool,
    #[serde(default = "default_worktree_limit")]
    detect_worktrees_limit: usize,
    #[serde(default)]
    autofetch: bool,
    #[serde(default)]
    branch_random_name: GitBranchRandomNameConfig,
}

impl Default for ScmConfig {
    fn default() -> Self {
        Self {
            default_view_mode: default_view_mode(),
            count_badge: default_count_badge(),
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            auto_repository_detection: true,
            untracked_changes: default_untracked_changes(),
            enable_smart_commit: false,
            suggest_smart_commit: true,
            confirm_sync: true,
            confirm_force_push: true,
            confirm_empty_commits: true,
            post_commit_command: default_post_commit_command(),
            show_action_button: true,
            detect_worktrees: true,
            detect_worktrees_limit: DEFAULT_WORKTREE_LIMIT,
            autofetch: false,
            branch_random_name: GitBranchRandomNameConfig::default(),
        }
    }
}

impl<'de> Deserialize<'de> for ScmConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawScmConfig::deserialize(deserializer)?;
        let config = Self {
            default_view_mode: raw.default_view_mode,
            count_badge: raw.count_badge,
        };
        config.validate().map_err(D::Error::custom)?;
        Ok(config)
    }
}

impl<'de> Deserialize<'de> for GitConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawGitConfig::deserialize(deserializer)?;
        let config = Self {
            auto_repository_detection: raw.auto_repository_detection,
            untracked_changes: raw.untracked_changes,
            enable_smart_commit: raw.enable_smart_commit,
            suggest_smart_commit: raw.suggest_smart_commit,
            confirm_sync: raw.confirm_sync,
            confirm_force_push: raw.confirm_force_push,
            confirm_empty_commits: raw.confirm_empty_commits,
            post_commit_command: raw.post_commit_command,
            show_action_button: raw.show_action_button,
            detect_worktrees: raw.detect_worktrees,
            detect_worktrees_limit: raw.detect_worktrees_limit,
            autofetch: raw.autofetch,
            branch_random_name: raw.branch_random_name,
        };
        config.validate().map_err(D::Error::custom)?;
        Ok(config)
    }
}

impl ScmConfig {
    /// 校验 Source Control 枚举配置。
    ///
    /// 返回:
    /// - 配置合法时返回成功，否则返回字段错误
    fn validate(&self) -> Result<(), String> {
        if !matches!(self.default_view_mode.as_str(), "list" | "tree") {
            return Err("scm.default_view_mode must be list or tree".to_string());
        }
        if !matches!(self.count_badge.as_str(), "all" | "focused" | "off") {
            return Err("scm.count_badge must be all, focused, or off".to_string());
        }
        Ok(())
    }
}

impl GitConfig {
    /// 校验 Git 枚举和数量配置。
    ///
    /// 返回:
    /// - 配置合法时返回成功，否则返回字段错误
    fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.untracked_changes.as_str(),
            "mixed" | "separate" | "hidden"
        ) {
            return Err("git.untracked_changes must be mixed, separate, or hidden".to_string());
        }
        if !matches!(self.post_commit_command.as_str(), "none" | "push" | "sync") {
            return Err("git.post_commit_command must be none, push, or sync".to_string());
        }
        if !(1..=128).contains(&self.detect_worktrees_limit) {
            return Err("git.detect_worktrees_limit must be between 1 and 128".to_string());
        }
        Ok(())
    }
}

fn default_true() -> bool {
    true
}

fn default_view_mode() -> String {
    "list".to_string()
}

fn default_count_badge() -> String {
    "all".to_string()
}

fn default_untracked_changes() -> String {
    "separate".to_string()
}

fn default_post_commit_command() -> String {
    "none".to_string()
}

fn default_worktree_limit() -> usize {
    DEFAULT_WORKTREE_LIMIT
}
