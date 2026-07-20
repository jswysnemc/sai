use super::tool_history::load_tool_exchanges_for_turn;
use super::turns::TurnStatus;
use super::{StateStore, ToolCallStatus};
use anyhow::Result;
use serde::Serialize;

/// 会话时间线中的消息。
#[derive(Debug, Clone, Serialize)]
pub struct TimelineMessage {
    pub timestamp: String,
    pub content: String,
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_urls: Vec<String>,
}

/// 会话时间线中的工具调用。
#[derive(Debug, Clone, Serialize)]
pub struct TimelineToolEntry {
    pub id: String,
    pub name: String,
    pub arguments: String,
    pub status: String,
    pub output: String,
    pub ok: Option<bool>,
    pub error: Option<String>,
    pub result_ref: Option<String>,
    pub original_chars: Option<usize>,
    pub created_at: String,
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<TimelinePermissionDecision>,
}

/// 历史工具调用对应的权限决定。
#[derive(Debug, Clone, Serialize)]
pub struct TimelinePermissionDecision {
    pub decision: String,
    pub reply: Option<String>,
}

/// 按轮次组织的会话时间线。
#[derive(Debug, Clone, Serialize)]
pub struct SessionTimelineTurn {
    pub turn_id: String,
    pub seq: i64,
    pub status: String,
    pub user: TimelineMessage,
    pub assistant: TimelineMessage,
    pub tools: Vec<TimelineToolEntry>,
    pub automatic: bool,
}

/// 会话时间线中展示的最新压缩摘要。
#[derive(Debug, Clone, Serialize)]
pub struct SessionTimelineCompaction {
    pub applied: bool,
    pub turn_count: usize,
    pub summary: String,
    pub created_at: String,
    pub reason: String,
}

/// 会话时间线响应，包含轮次与可选压缩摘要。
#[derive(Debug, Clone, Serialize)]
pub struct SessionTimeline {
    pub turns: Vec<SessionTimelineTurn>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compaction: Option<SessionTimelineCompaction>,
}

impl StateStore {
    /// 读取最近会话轮次及其结构化工具历史。
    ///
    /// 参数:
    /// - `limit`: 最大轮次数量
    ///
    /// 返回:
    /// - 按对话顺序排列的会话时间线
    pub fn session_timeline(&self, limit: usize) -> Result<Vec<SessionTimelineTurn>> {
        let mut turns = self.conv_db.load_turns()?;
        let start = turns.len().saturating_sub(limit);
        let turns = turns.split_off(start);
        turns
            .into_iter()
            .map(|turn| {
                let exchanges =
                    load_tool_exchanges_for_turn(&self.conv_db, &self.session_id, &turn.turn_id)?;
                let tools = exchanges
                    .into_iter()
                    .map(|exchange| TimelineToolEntry {
                        id: exchange.call.provider_call_id,
                        name: exchange.call.tool_name,
                        arguments: exchange.call.arguments,
                        status: tool_status(&exchange.call.status).to_string(),
                        output: exchange
                            .result
                            .as_ref()
                            .map(|result| result.result_preview.clone())
                            .unwrap_or_default(),
                        ok: exchange.result.as_ref().map(|result| result.ok),
                        error: exchange
                            .result
                            .as_ref()
                            .and_then(|result| result.error.clone()),
                        result_ref: exchange
                            .result
                            .as_ref()
                            .and_then(|result| result.result_ref.clone()),
                        original_chars: exchange
                            .result
                            .as_ref()
                            .map(|result| result.original_chars),
                        created_at: exchange.call.created_at,
                        completed_at: exchange.result.map(|result| result.completed_at),
                        permission: None,
                    })
                    .collect();
                let automatic = is_automatic_input(&turn.user_content);
                Ok(SessionTimelineTurn {
                    turn_id: turn.turn_id,
                    seq: turn.seq,
                    status: turn_status(turn.status).to_string(),
                    user: TimelineMessage {
                        timestamp: turn.user_timestamp,
                        content: turn.user_content,
                        reasoning: None,
                        image_urls: turn.user_image_urls,
                    },
                    assistant: TimelineMessage {
                        timestamp: turn.assistant_timestamp.unwrap_or_default(),
                        content: turn.assistant_content,
                        reasoning: turn.assistant_reasoning,
                        image_urls: Vec::new(),
                    },
                    tools,
                    automatic,
                })
            })
            .collect()
    }

    /// 读取会话时间线，并附带最新压缩摘要（若存在）。
    ///
    /// 参数:
    /// - `limit`: 最大轮次数量
    ///
    /// 返回:
    /// - 轮次列表与可选压缩摘要
    pub fn session_timeline_with_compaction(&self, limit: usize) -> Result<SessionTimeline> {
        Ok(SessionTimeline {
            turns: self.session_timeline(limit)?,
            compaction: self.latest_timeline_compaction()?,
        })
    }

    /// 读取最新 checkpoint 作为时间线压缩展示。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 有摘要时的压缩展示数据
    fn latest_timeline_compaction(&self) -> Result<Option<SessionTimelineCompaction>> {
        let checkpoint = {
            let conn = self.conv_db.conn.lock().unwrap();
            crate::state::checkpoints::load_latest_checkpoint(&conn)?
        };
        let Some(checkpoint) = checkpoint else {
            return Ok(None);
        };
        let summary = checkpoint.summary.trim();
        if summary.is_empty() {
            return Ok(None);
        }
        Ok(Some(SessionTimelineCompaction {
            applied: true,
            turn_count: checkpoint.source_turn_count,
            summary: summary.to_string(),
            created_at: checkpoint.created_at,
            reason: match checkpoint.reason {
                crate::state::checkpoints::CheckpointReason::Auto => "auto",
                crate::state::checkpoints::CheckpointReason::Manual => "manual",
                crate::state::checkpoints::CheckpointReason::Legacy => "legacy",
            }
            .to_string(),
        }))
    }
}

/// 判断时间线中的用户输入是否由 Sai 自动提交。
///
/// 参数:
/// - `content`: 持久化的轮次用户输入
///
/// 返回:
/// - Goal 续轮或外部完成事件返回 true
fn is_automatic_input(content: &str) -> bool {
    crate::goal::is_continuation_input(content)
        || content
            .trim_start()
            .starts_with("<external-completion-events>")
}

/// 将工具状态转换为 Web 稳定文本。
///
/// 参数:
/// - `status`: 工具调用状态
///
/// 返回:
/// - 状态文本
fn tool_status(status: &ToolCallStatus) -> &'static str {
    match status {
        ToolCallStatus::Pending => "running",
        ToolCallStatus::Completed => "completed",
        ToolCallStatus::Error | ToolCallStatus::Interrupted => "failed",
    }
}

/// 将轮次状态转换为 Web 稳定文本。
///
/// 参数:
/// - `status`: 对话轮次状态
///
/// 返回:
/// - 状态文本
fn turn_status(status: TurnStatus) -> &'static str {
    match status {
        TurnStatus::Running => "running",
        TurnStatus::Completed => "completed",
        TurnStatus::Interrupted => "interrupted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::SaiPaths;
    use std::path::PathBuf;

    /// 创建时间线测试所需路径。
    ///
    /// 参数:
    /// - `root`: 临时根目录
    ///
    /// 返回:
    /// - Sai 路径集合
    fn test_paths(root: PathBuf) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }

    #[test]
    fn groups_tool_history_with_its_turn() {
        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        store.start_turn("turn_1", "inspect").unwrap();
        store
            .record_tool_call_started("turn_1", 0, "call_1", "run_command", "{}")
            .unwrap();
        store
            .record_tool_result_completed("turn_1", "call_1", true, "ok", None, None, 2)
            .unwrap();
        store.complete_turn("turn_1", "done", None).unwrap();

        let timeline = store.session_timeline(10).unwrap();

        assert_eq!(timeline.len(), 1);
        assert_eq!(timeline[0].tools.len(), 1);
        assert_eq!(timeline[0].tools[0].name, "run_command");
        assert_eq!(timeline[0].tools[0].output, "ok");
    }

    #[test]
    fn marks_goal_continuation_turns_as_automatic() {
        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        store
            .start_turn(
                "turn_goal",
                "<goal-continuation goal_id=\"goal_test\">continue</goal-continuation>",
            )
            .unwrap();
        store.complete_turn("turn_goal", "progress", None).unwrap();

        let timeline = store.session_timeline(10).unwrap();

        assert_eq!(timeline.len(), 1);
        assert!(timeline[0].automatic);
    }

    #[test]
    fn marks_external_completion_turns_as_automatic() {
        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        store
            .start_turn(
                "turn_external",
                "<external-completion-events>subagent done</external-completion-events>",
            )
            .unwrap();
        store.complete_turn("turn_external", "continued", None).unwrap();

        let timeline = store.session_timeline(10).unwrap();

        assert!(timeline[0].automatic);
    }

    #[test]
    fn includes_latest_checkpoint_summary_in_timeline() {
        use crate::llm::ChatMessage;

        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        for index in 1..=4 {
            let turn_id = format!("turn_{index}");
            store.start_turn(&turn_id, &"u".repeat(200)).unwrap();
            store
                .complete_turn(&turn_id, &"a".repeat(200), None)
                .unwrap();
        }
        let messages = vec![ChatMessage::plain("user", "x".repeat(8_000))];
        let request = store
            .select_compaction_for_messages(&messages, 2_000, true)
            .unwrap()
            .expect("compaction request");
        store
            .apply_compaction(&request, "## Goal\n- keep context")
            .unwrap();

        let timeline = store.session_timeline_with_compaction(10).unwrap();
        let compaction = timeline.compaction.expect("compaction present");
        assert!(compaction.applied);
        assert!(compaction.summary.contains("keep context"));
        assert!(compaction.turn_count >= 1);
    }
}
