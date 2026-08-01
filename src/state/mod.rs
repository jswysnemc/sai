mod checkpoints;
mod compaction;
mod context_epoch;
pub(crate) mod failure_recovery;
mod goals;
pub(crate) mod input_history;
mod loaded_tools;
mod pending_turn;
pub(crate) mod request_projection;
mod runtime_recovery;
pub(crate) mod session_memory;
mod session_snapshot;
mod session_timeline;
mod sessions;
mod store_context_epoch;
mod store_usage;
pub(crate) mod tool_history;
mod turns;
mod usage;
pub(crate) mod worktree_undo;

#[cfg(test)]
use crate::llm::Usage;
use crate::paths::SaiPaths;
use anyhow::Result;
#[cfg(test)]
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::path::PathBuf;
use std::sync::Arc;

#[allow(unused_imports)]
pub use compaction::{
    classify_context_pressure, compaction_trigger_chars, CompactionApplyOutcome, CompactionRequest,
    CompactionSummary, ContextPressure,
};
pub(crate) use compaction::{summary_char_limit, validate_summary};
pub use context_epoch::{ContextEpochProjection, ContextEpochSummary, ContextSourceInput};
pub use failure_recovery::{FailureKind, RecoverySnapshot, RecoveryStatus};
pub use pending_turn::PendingTurnGuard;
#[allow(unused_imports)]
pub use session_memory::summary::SessionMemorySummary;
pub use session_snapshot::{context_ratio, ActiveRunSummary, SessionSnapshot};
#[allow(unused_imports)]
pub use session_timeline::{
    SessionTimeline, SessionTimelineCompaction, SessionTimelineTurn, TimelineMessage,
    TimelinePermissionDecision, TimelineToolEntry,
};
#[allow(unused_imports)]
pub use sessions::{
    active_session_id_for_workspace, active_state_dir, create_session,
    create_session_for_workspace, delete_session, delete_sessions,
    ensure_active_session as active_session, ensure_workspace_session, fork_session_until_turn,
    list_sessions, list_sessions_for_workspace, locate_session_dirs, rename_session,
    state_dir_for_workspace_session, switch_session, title_from_message_public,
    workspace_id_for_path, SessionInfo,
};
#[allow(unused_imports)]
pub use tool_history::{
    ToolCallStatus, ToolHistorySummary, ToolResultMaintenanceMode, ToolResultMaintenanceStats,
};
#[cfg(test)]
pub use turns::TurnStatus;
pub use turns::{turns_to_entries, ConversationDb, SessionTree, StoredConversationEntry, Turn, TurnTreeNode};
pub use usage::UsageSnapshot;
/// 撤销最后一轮对话及其工作树修改后的结果。
#[derive(Debug, Clone)]
pub struct UndoOutcome {
    pub removed: usize,
    pub prompt: Option<String>,
    pub worktree_restored: bool,
}

/// 仅回滚会话上下文后的结果，不修改工具已经产生的工作树副作用。
#[derive(Debug, Clone)]
pub struct ContextRollbackOutcome {
    pub removed: usize,
    pub prompt: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StateStore {
    base_state_dir: PathBuf,
    session_id: String,
    state_dir: PathBuf,
    conv_db: Arc<ConversationDb>,
}

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
    /// 三个构造入口共用：迁移历史格式，并收敛上次运行遗留的撤销快照。
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
    /// 进程被强制结束时快照收尾不会执行，`pending_` 目录会永久留在磁盘上；
    /// 已完成快照也需要按保留上限淘汰。清理失败不应阻断会话打开，因此忽略错误。
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
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 会话 ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 返回当前会话 TODO 状态文件。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - TODO 状态文件路径
    pub(crate) fn todo_file(&self) -> PathBuf {
        self.state_dir.join("todos.json")
    }

    /// 系统提示变化时重置会话。
    ///
    /// 参数:
    /// - `system_prompt`: 当前系统提示
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

    /// 开始新对话轮次。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `content`: 已生成的部分助手正文
    /// - `reasoning`: 可选的部分推理内容
    /// - `user_content`: 用户输入
    ///
    /// 返回:
    /// - 写入是否成功
    #[allow(dead_code)]
    pub fn start_turn(&self, turn_id: &str, user_content: &str) -> Result<()> {
        self.start_turn_with_images(turn_id, user_content, &[])
    }

    /// 开始对话轮次并持久化用户图片附件。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `user_content`: 用户输入
    /// - `user_image_urls`: 用户附件图片 data URL 列表
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn start_turn_with_images(
        &self,
        turn_id: &str,
        user_content: &str,
        user_image_urls: &[String],
    ) -> Result<()> {
        self.conv_db
            .start_turn_with_images(turn_id, user_content, user_image_urls)?;
        sessions::touch_session_with_message(&self.base_state_dir, &self.session_id, user_content)
    }

    /// 完成对话轮次。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `content`: 助手回复
    /// - `reasoning`: 可选推理内容
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn complete_turn(
        &self,
        turn_id: &str,
        content: &str,
        reasoning: Option<&str>,
    ) -> Result<()> {
        self.conv_db.complete_turn(turn_id, content, reasoning)?;
        let _ = session_memory::extractor::extract_after_turn_with_default_summary(
            &self.conv_db,
            &self.session_id,
        );
        Ok(())
    }

    /// 将耗时写入最近一轮对话。
    ///
    /// 参数:
    /// - `duration_ms`: 处理耗时毫秒
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn set_last_turn_duration_ms(&self, duration_ms: u64) -> Result<()> {
        let Some((turn_id, _)) = self.conv_db.last_turn_identity()? else {
            return Ok(());
        };
        self.conv_db.set_turn_duration_ms(&turn_id, duration_ms)
    }

    /// 中断对话轮次。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn interrupt_turn(
        &self,
        turn_id: &str,
        content: &str,
        reasoning: Option<&str>,
    ) -> Result<()> {
        self.conv_db.interrupt_turn(turn_id, content, reasoning)?;
        self.settle_pending_tool_calls_for_turns(&[turn_id.to_string()])?;
        Ok(())
    }

    /// 失败对话轮次。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `content`: 已生成的部分正文
    /// - `reasoning`: 可选推理内容
    /// - `error`: 失败原因
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn fail_turn(
        &self,
        turn_id: &str,
        content: &str,
        reasoning: Option<&str>,
        error: &str,
    ) -> Result<()> {
        self.conv_db.fail_turn(turn_id, content, reasoning, error)?;
        self.settle_pending_tool_calls_for_turns(&[turn_id.to_string()])?;
        Ok(())
    }

    /// 附加工具报告上下文。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `tool_name`: 工具名称
    /// - `report`: 工具报告
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn append_tool_report_context(
        &self,
        turn_id: &str,
        tool_name: &str,
        report: &str,
    ) -> Result<()> {
        self.conv_db.append_tool_report(
            turn_id,
            &format!(
                "<previous_tool_report name=\"{tool_name}\">\n{}\n</previous_tool_report>",
                report.trim()
            ),
        )
    }

    /// 恢复运行中的旧轮次为中断状态。
    ///
    /// 返回:
    /// - 被恢复轮次数量
    pub fn recover_stale_turns(&self) -> Result<usize> {
        let turn_ids = self.conv_db.recover_stale_running_turns()?;
        let settled_tools = self.settle_pending_tool_calls_for_turns(&turn_ids)?;
        for turn_id in &turn_ids {
            self.record_recovery_failure(
                Some(turn_id),
                FailureKind::StaleRunningTurn,
                RecoveryStatus::Resolved,
                "启动时发现运行中轮次，已按中断语义恢复",
                0,
                0,
                0,
            )?;
        }
        if settled_tools > 0 {
            self.record_recovery_failure(
                None,
                FailureKind::ToolHistoryPendingStale,
                RecoveryStatus::Resolved,
                &format!("启动时发现 {settled_tools} 个未完成工具调用，已标记为中断"),
                0,
                0,
                0,
            )?;
        }
        Ok(turn_ids.len())
    }

    /// 兼容旧 JSONL 孤立用户消息检查。
    ///
    /// 返回:
    /// - 是否标记了中断轮次
    #[cfg(test)]
    pub fn mark_interrupted_turn_if_needed(&self) -> Result<bool> {
        let recovered = self.recover_stale_turns()?;
        Ok(recovered > 0)
    }

    /// 读取最近历史入口。
    ///
    /// 参数:
    /// - `limit`: 最大入口数量
    ///
    /// 返回:
    /// - 历史入口
    pub fn history(&self, limit: usize) -> Result<Vec<StoredConversationEntry>> {
        let mut entries = self.load_conversation()?;
        let start = entries.len().saturating_sub(limit);
        Ok(entries.split_off(start))
    }

    /// 读取完整对话历史入口。
    ///
    /// 返回:
    /// - 旧消息入口视图
    pub fn load_conversation(&self) -> Result<Vec<StoredConversationEntry>> {
        Ok(turns_to_entries(self.conv_db.active_branch_turns()?))
    }

    /// 判断最后一轮是否为指定输入对应的部分中断回复。
    ///
    /// 参数:
    /// - `input`: 本轮用户输入
    ///
    /// 返回:
    /// - 最后一轮匹配且包含部分助手正文时返回 true
    pub(crate) fn latest_interrupted_turn_has_content(&self, input: &str) -> Result<bool> {
        Ok(self.conv_db.active_branch_turns()?.last().is_some_and(|turn| {
            turn.status == turns::TurnStatus::Interrupted
                && turn.user_content.trim() == input.trim()
                && !turn.assistant_content.trim().is_empty()
        }))
    }

    /// 读取当前活动分支上的对话轮次。
    ///
    /// 会话是一棵树，历史指的是从根到活动叶子的那一条路径。
    /// 其它分支的轮次仍保存在库中，但不参与当前上下文。
    ///
    /// 返回:
    /// - 活动分支轮次列表，按时间升序
    pub fn load_turns(&self) -> Result<Vec<Turn>> {
        self.conv_db.active_branch_turns()
    }

    /// 读取当前会话树。
    ///
    /// 返回:
    /// - 含全部分支与活动叶子的树视图
    pub fn session_tree(&self) -> Result<turns::SessionTree> {
        self.conv_db.session_tree()
    }

    /// 把活动叶子切换到指定轮次。
    ///
    /// 参数:
    /// - `turn_id`: 目标轮次
    ///
    /// 返回:
    /// - 切换是否成功
    pub fn switch_active_leaf(&self, turn_id: &str) -> Result<()> {
        self.conv_db.switch_active_leaf(turn_id)
    }

    /// 把活动叶子退回指定轮次的父轮次。
    ///
    /// 参数:
    /// - `turn_id`: 要退出的轮次
    ///
    /// 返回:
    /// - 退回后的活动叶子；已在根部时返回 None
    pub fn move_leaf_to_parent(&self, turn_id: &str) -> Result<Option<String>> {
        self.conv_db.move_leaf_to_parent(turn_id)
    }

    /// 读取会话内全部轮次，含所有分支。
    ///
    /// 仅用于树视图、清理与统计；构造对话上下文一律使用 `load_turns`。
    ///
    /// 返回:
    /// - 全部轮次列表，按 seq 升序
    pub fn load_all_turns(&self) -> Result<Vec<Turn>> {
        self.conv_db.load_turns()
    }

    /// 构造当前会话历史投影。
    ///
    /// 参数:
    /// - `exclude_turn_id`: 可选排除的运行中轮次
    ///
    /// 返回:
    /// - 会话历史投影
    pub(crate) fn project_history(
        &self,
        exclude_turn_id: Option<&str>,
    ) -> Result<checkpoints::ProjectedHistory> {
        checkpoints::project_history(&self.conv_db, &self.session_id, exclude_turn_id)
    }

    /// 清空对话历史。
    ///
    /// 返回:
    /// - 清空是否成功
    pub fn reset_conversation(&self) -> Result<()> {
        self.conv_db.reset()?;
        self.clear_loaded_tools()?;
        self.clear_compaction_summary()?;
        self.clear_last_usage()
    }

    /// 读取当前会话已经载入的工具集合。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 已载入工具名称列表
    pub fn load_loaded_tools(&self) -> Result<Vec<String>> {
        loaded_tools::load(&self.loaded_tools_file())
    }

    /// 保存当前会话已经载入的工具集合。
    ///
    /// 参数:
    /// - `names`: 已载入工具名称列表
    ///
    /// 返回:
    /// - 保存是否成功
    pub fn save_loaded_tools(&self, names: &[String]) -> Result<()> {
        loaded_tools::save(&self.loaded_tools_file(), names)
    }

    /// 清空当前会话已经载入的工具集合。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 清空是否成功
    pub fn clear_loaded_tools(&self) -> Result<()> {
        loaded_tools::clear(&self.loaded_tools_file())
    }

    /// 撤销最后一轮对话。
    ///
    /// 返回:
    /// - 删除轮次数量和被撤销的用户输入
    pub fn undo_last_turn(&self) -> Result<UndoOutcome> {
        let Some((turn_id, _)) = self.conv_db.last_turn_identity()? else {
            return Ok(UndoOutcome {
                removed: 0,
                prompt: None,
                worktree_restored: false,
            });
        };
        let worktree = worktree_undo::restore_latest_snapshot(&self.state_dir, &turn_id)?;
        let (removed, prompt) = self.conv_db.undo_last_turn()?;
        Ok(UndoOutcome {
            removed,
            prompt,
            worktree_restored: worktree.restored,
        })
    }

    /// 仅恢复指定轮次的工作树改动，不删除对话历史。
    ///
    /// 参数:
    /// - `expected_turn_id`: 轮次标识
    /// - `paths`: 目标路径；空表示整轮
    ///
    /// 返回:
    /// - 是否恢复了工作树
    pub fn restore_turn_worktree(&self, expected_turn_id: &str, paths: &[String]) -> Result<bool> {
        let outcome =
            worktree_undo::restore_snapshot_paths(&self.state_dir, expected_turn_id, paths)?;
        Ok(outcome.restored)
    }

    /// 回滚与预期标识匹配的最后一轮会话上下文。
    ///
    /// 参数:
    /// - `expected_turn_id`: 准备重试的最后一轮标识
    ///
    /// 返回:
    /// - 删除数量和原用户输入，不恢复工作树
    pub fn rollback_last_turn_context(
        &self,
        expected_turn_id: &str,
    ) -> Result<ContextRollbackOutcome> {
        let (removed, prompt) = self.conv_db.rollback_last_turn(expected_turn_id)?;
        if removed > 0 {
            worktree_undo::discard_turn_snapshot(self, expected_turn_id)?;
        }
        Ok(ContextRollbackOutcome { removed, prompt })
    }

    /// 是否存在运行中轮次。
    ///
    /// 返回:
    /// - 是否存在运行中轮次
    #[allow(dead_code)]
    pub fn has_running_turns(&self) -> Result<bool> {
        self.conv_db.has_running_turns()
    }

    /// 从旧 JSONL 文件迁移历史。
    ///
    /// 返回:
    /// - 迁移轮次数量
    pub fn migrate_from_jsonl(&self) -> Result<usize> {
        self.conv_db.migrate_from_jsonl(&self.conversation_file())
    }

    /// 兼容旧测试和辅助代码追加消息。
    ///
    /// 参数:
    /// - `role`: 消息角色
    /// - `content`: 消息内容
    ///
    /// 返回:
    /// - 写入是否成功
    #[cfg(test)]
    pub fn append_message(&self, role: &str, content: &str) -> Result<()> {
        match role {
            "user" => self.start_turn(&compat_turn_id(), content),
            "assistant" => self.append_assistant_message(content, None),
            _ => Ok(()),
        }
    }

    /// 兼容旧测试和辅助代码追加助手消息。
    ///
    /// 参数:
    /// - `content`: 助手回复
    /// - `reasoning`: 可选推理内容
    ///
    /// 返回:
    /// - 写入是否成功
    #[cfg(test)]
    pub fn append_assistant_message(&self, content: &str, reasoning: Option<&str>) -> Result<()> {
        if let Some(turn) = self
            .conv_db
            .load_turns()?
            .into_iter()
            .rev()
            .find(|turn| turn.status == TurnStatus::Running)
        {
            self.complete_turn(&turn.turn_id, content, reasoning)?;
        }
        Ok(())
    }

    /// 测试用：写入损坏的 Context Epoch snapshot。
    ///
    /// 参数: `snapshot_json` 是损坏的 snapshot JSON
    /// 返回: 写入是否成功
    #[cfg(test)]
    pub fn corrupt_context_epoch_snapshot_for_test(&self, snapshot_json: &str) -> Result<()> {
        let conn = self.conv_db.conn.lock().unwrap();
        conn.execute(
            "UPDATE context_epochs SET snapshot_json = ?1 WHERE session_id = ?2",
            rusqlite::params![snapshot_json, self.session_id],
        )?;
        Ok(())
    }

    fn conversation_file(&self) -> PathBuf {
        self.state_dir.join("conversation.jsonl")
    }

    fn usage_file(&self) -> PathBuf {
        self.state_dir.join("usage.json")
    }

    fn loaded_tools_file(&self) -> PathBuf {
        self.state_dir.join("loaded-tools.json")
    }

    fn log_file(&self) -> PathBuf {
        self.state_dir.join("sai.log")
    }

    fn profile_file(&self) -> PathBuf {
        self.state_dir.join("profile.md")
    }

    fn compaction_summary_file(&self) -> PathBuf {
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

/// 创建兼容写入使用的轮次标识。
///
/// 返回:
/// - 轮次标识
#[cfg(test)]
fn compat_turn_id() -> String {
    format!(
        "compat_{}_{}",
        Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
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

#[cfg(test)]
mod tests;
