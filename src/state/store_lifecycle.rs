use super::{checkpoints, context_epoch, sessions, worktree_undo, ConversationDb, StateStore};
use crate::paths::SaiPaths;
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::Arc;

impl StateStore {
    /// 创建状态存储并迁移旧对话历史。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    ///
    /// 返回:
    /// - 状态存储
    pub fn new(paths: &SaiPaths) -> Result<Self> {
        let session = sessions::ensure_active_session(paths)?;
        let base_state_dir = sessions::session_scope_dir(paths)?;
        let state_dir = sessions::active_state_dir(paths)?;
        let conv_db = Arc::new(ConversationDb::open(&state_dir)?);
        let store = Self {
            base_state_dir,
            session_id: session.id,
            state_dir,
            conv_db,
        };
        store.prepare_after_open()?;
        Ok(store)
    }

    /// 打开会话状态后的统一收尾。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 迁移是否成功；快照清理失败不阻断打开
    fn prepare_after_open(&self) -> Result<()> {
        self.migrate_from_jsonl()?;
        checkpoints::migrate_legacy_compaction_summary(self)?;
        self.cleanup_worktree_snapshots();
        Ok(())
    }

    /// 清理本会话的工作树撤销快照残留。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    fn cleanup_worktree_snapshots(&self) {
        let root = worktree_undo::snapshot_root(&self.state_dir);
        let _ = worktree_undo::cleanup_snapshot_root(&root);
    }

    /// 创建绑定到指定会话的状态存储，不修改全局当前会话。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    /// - `session_id`: 会话 ID
    ///
    /// 返回:
    /// - 指定会话状态存储
    pub fn for_session(paths: &SaiPaths, session_id: &str) -> Result<Self> {
        let (base_state_dir, state_dir) = sessions::locate_session_dirs(paths, session_id)?;
        let conv_db = Arc::new(ConversationDb::open(&state_dir)?);
        let store = Self {
            base_state_dir,
            session_id: session_id.trim().to_string(),
            state_dir,
            conv_db,
        };
        store.prepare_after_open()?;
        Ok(store)
    }

    /// 创建绑定到指定工作区和会话的状态存储。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    /// - `workspace_path`: 工作区目录
    /// - `session_id`: 会话 ID
    ///
    /// 返回:
    /// - 指定会话状态存储
    pub fn for_workspace_session(
        paths: &SaiPaths,
        workspace_path: &std::path::Path,
        session_id: &str,
    ) -> Result<Self> {
        let (base_state_dir, state_dir) =
            sessions::state_dir_for_workspace_session(paths, workspace_path, session_id)?;
        let conv_db = Arc::new(ConversationDb::open(&state_dir)?);
        let store = Self {
            base_state_dir,
            session_id: session_id.trim().to_string(),
            state_dir,
            conv_db,
        };
        store.prepare_after_open()?;
        Ok(store)
    }

    /// 返回当前会话状态目录。
    ///
    /// 返回:
    /// - 状态目录路径
    pub(crate) fn state_dir(&self) -> &std::path::Path {
        &self.state_dir
    }

    /// 初始化状态文件。
    ///
    /// 返回:
    /// - 初始化是否成功
    pub fn init_files(&self) -> Result<()> {
        std::fs::create_dir_all(&self.state_dir)?;
        if !self.usage_file().exists() {
            std::fs::write(self.usage_file(), "{\n  \"requests\": 0,\n  \"prompt_tokens\": 0,\n  \"completion_tokens\": 0,\n  \"total_tokens\": 0\n}\n")?;
        }
        touch(self.log_file())?;
        if !self.profile_file().exists() {
            std::fs::write(self.profile_file(), "# Sai Profile\n\n")?;
        }
        Ok(())
    }

    /// 返回当前状态存储对应的会话 ID。
    ///
    /// 返回:
    /// - 会话 ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 返回当前会话 TODO 状态文件。
    ///
    /// 返回:
    /// - TODO 状态文件路径
    pub(crate) fn todo_file(&self) -> PathBuf {
        self.state_dir.join("todos.json")
    }

    /// 系统提示变化时重置会话。
    ///
    /// 参数:
    /// - `system_prompt`: 系统提示
    ///
    /// 返回:
    /// - 重置检查是否成功
    pub fn reset_if_prompt_changed(&self, system_prompt: &str) -> Result<()> {
        self.init_files()?;
        let fingerprint = prompt_fingerprint(system_prompt);
        let file = self.prompt_fingerprint_file();
        let previous = std::fs::read_to_string(&file).unwrap_or_default();
        context_epoch::prepare_context_epoch(&self.conv_db, &self.session_id, system_prompt)?;
        if previous.trim() != fingerprint {
            std::fs::write(file, format!("{fingerprint}\n"))?;
        }
        Ok(())
    }

    pub(super) fn conversation_file(&self) -> PathBuf {
        self.state_dir.join("conversation.jsonl")
    }

    pub(super) fn usage_file(&self) -> PathBuf {
        self.state_dir.join("usage.json")
    }

    pub(super) fn loaded_tools_file(&self) -> PathBuf {
        self.state_dir.join("loaded-tools.json")
    }

    pub(super) fn loaded_skills_file(&self) -> PathBuf {
        self.state_dir.join("loaded-skills.json")
    }

    fn log_file(&self) -> PathBuf {
        self.state_dir.join("sai.log")
    }

    fn profile_file(&self) -> PathBuf {
        self.state_dir.join("profile.md")
    }

    pub(super) fn compaction_summary_file(&self) -> PathBuf {
        self.state_dir.join("compaction-summary.json")
    }

    fn prompt_fingerprint_file(&self) -> PathBuf {
        self.state_dir.join("prompt.sha256")
    }
}

/// 计算系统提示指纹。
///
/// 参数:
/// - `system_prompt`: 系统提示
///
/// 返回:
/// - 十六进制指纹
fn prompt_fingerprint(system_prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(system_prompt.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// 确保文件存在。
///
/// 参数:
/// - `path`: 文件路径
///
/// 返回:
/// - 创建是否成功
fn touch(path: PathBuf) -> Result<()> {
    OpenOptions::new().create(true).append(true).open(path)?;
    Ok(())
}
