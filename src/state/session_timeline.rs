use super::tool_history::load_tool_exchanges_for_turn;
use super::turns::TurnStatus;
use super::{StateStore, ToolCallStatus};
use crate::llm::Usage;
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
    pub seq: usize,
    /// 产生该调用的模型子轮编号；同轮内它变化即代表又发了一次模型请求
    pub assistant_round: usize,
    pub name: String,
    pub arguments: String,
    pub status: String,
    pub output: String,
    /// 决定这次调用的模型思考；同一子轮的多次调用共享同一份
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    pub ok: Option<bool>,
    pub error: Option<String>,
    pub result_ref: Option<String>,
    pub original_chars: Option<usize>,
    pub created_at: String,
    pub completed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission: Option<TimelinePermissionDecision>,
}

/// 会话时间线中的轮次内消息。
#[derive(Debug, Clone, Serialize)]
pub struct TimelineTurnMessage {
    pub id: String,
    pub seq: usize,
    pub after_tool_seq: usize,
    pub kind: String,
    pub role: String,
    pub content: String,
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub image_urls: Vec<String>,
    pub created_at: String,
}

/// 历史工具调用对应的权限决定。
#[derive(Debug, Clone, Serialize)]
pub struct TimelinePermissionDecision {
    pub decision: String,
    /// 拒绝时回复给模型的原因
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply: Option<String>,
    /// 允许来源；`auto_audit` 表示由审核模型放行
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// 自动审核给出的放行理由
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<TimelineTurnMessage>,
    pub automatic: bool,
    /// 处理耗时毫秒；0 表示历史数据未记录
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub duration_ms: u64,
    /// 首字延迟毫秒；0 表示历史数据未记录
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub ttft_ms: u64,
    /// 同一轮全部模型请求的汇总用量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
    /// 失败轮的错误摘要；正文与错误各归其位，前端不再拿正文当失败详情
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 写入供应商的用户消息相对可见正文多出来的注入前缀
    #[serde(skip_serializing_if = "Option::is_none")]
    pub injected_content: Option<String>,
}

/// 会话时间线中展示的最新压缩摘要。
#[derive(Debug, Clone, Serialize)]
pub struct SessionTimelineCompaction {
    pub applied: bool,
    pub turn_count: usize,
    /// 本次摘要覆盖的起始轮次序号；被删掉的旧轮次仍按此对齐
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub compacted_from_seq: i64,
    /// 本次摘要覆盖的结束轮次序号
    #[serde(default, skip_serializing_if = "is_zero_i64")]
    pub compacted_to_seq: i64,
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
        let mut turns = self.conv_db.active_branch_turns()?;
        let start = turns.len().saturating_sub(limit);
        let turns = turns.split_off(start);
        turns
            .into_iter()
            .map(|turn| {
                let usage = self.conv_db.turn_usage(&turn.turn_id)?;
                let ttft_ms = self.conv_db.turn_ttft_ms(&turn.turn_id)?.unwrap_or(0);
                let exchanges =
                    load_tool_exchanges_for_turn(&self.conv_db, &self.session_id, &turn.turn_id)?;
                let messages = self
                    .turn_messages(&turn.turn_id)?
                    .into_iter()
                    .map(|message| TimelineTurnMessage {
                        id: message.id,
                        seq: message.seq,
                        after_tool_seq: message.after_tool_seq,
                        kind: message.kind.as_str().to_string(),
                        role: message.kind.role().to_string(),
                        content: message.display_content,
                        reasoning: message.reasoning,
                        image_urls: message.image_urls,
                        created_at: message.created_at,
                    })
                    .collect();
                let tools = exchanges
                    .into_iter()
                    .map(|exchange| {
                        let name = exchange
                            .call
                            .display_tool_name
                            .clone()
                            .unwrap_or_else(|| exchange.call.tool_name.clone());
                        let arguments = exchange
                            .call
                            .display_arguments
                            .clone()
                            .unwrap_or_else(|| exchange.call.arguments.clone());
                        TimelineToolEntry {
                            id: exchange.call.provider_call_id,
                            seq: exchange.call.seq,
                            assistant_round: exchange.call.assistant_round,
                            name,
                            arguments,
                            status: tool_status(&exchange.call.status).to_string(),
                            output: exchange
                                .result
                                .as_ref()
                                .map(|result| result.result_preview.clone())
                                .unwrap_or_default(),
                            reasoning: exchange
                                .call
                                .assistant_reasoning
                                .filter(|value| !value.trim().is_empty()),
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
                        }
                    })
                    .collect();
                let automatic = is_automatic_input(&turn.user_content);
                let provider = self
                    .conv_db
                    .provider_user_content(&turn.turn_id, &turn.user_content)?;
                let injected_content = injected_prefix(&provider, &turn.user_content);
                // 无正文的失败轮 assistant_content 存的就是错误摘要（旧展示兼容），
                // 时间线里错误由 error 字段承载，正文清空避免气泡与错误框重复
                let assistant_content = if turn
                    .error
                    .as_deref()
                    .is_some_and(|error| error == turn.assistant_content.trim())
                {
                    String::new()
                } else {
                    turn.assistant_content
                };
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
                        content: assistant_content,
                        reasoning: turn.assistant_reasoning,
                        image_urls: Vec::new(),
                    },
                    tools,
                    messages,
                    automatic,
                    duration_ms: turn.duration_ms,
                    ttft_ms,
                    usage,
                    error: turn.error,
                    injected_content,
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
            compacted_from_seq: checkpoint.compacted_from_seq,
            compacted_to_seq: checkpoint.compacted_to_seq,
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
        TurnStatus::Failed => "failed",
    }
}

/// 判断 u64 是否为零，供 serde 跳过默认字段。
///
/// 参数:
/// - `value`: 数值
///
/// 返回:
/// - 是否为零
fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

/// 判断 i64 是否为零，供 serde 跳过缺省压缩区间。
fn is_zero_i64(value: &i64) -> bool {
    *value == 0
}

/// 取出供应商用户消息相对可见正文多出来的注入前缀。
///
/// 参数:
/// - `provider`: 实际发给模型的用户消息
/// - `visible`: 界面展示的用户输入
///
/// 返回:
/// - 注入前缀；两者相同时为 None
fn injected_prefix(provider: &str, visible: &str) -> Option<String> {
    let provider = provider.trim();
    let visible = visible.trim();
    if provider.is_empty() || provider == visible {
        return None;
    }
    if let Some(stripped) = provider.strip_suffix(visible) {
        let prefix = stripped.trim();
        if !prefix.is_empty() {
            return Some(prefix.to_string());
        }
    }
    Some(provider.to_string())
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

    /// 【工具历史】【渐进加载】验证界面显示真实工具且供应商投影保留统一网关。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn separates_display_tool_from_provider_gateway_call() {
        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        store.start_turn("turn_1", "inspect").unwrap();
        store
            .record_tool_call_started(
                "turn_1",
                0,
                "call_1",
                "invoke_tool",
                r#"{"tool_name":"read_file","arguments":{"path":"README.md"}}"#,
            )
            .unwrap();
        store
            .record_tool_call_display("call_1", "read_file", r#"{"path":"README.md"}"#)
            .unwrap();
        store
            .record_tool_result_completed("turn_1", "call_1", true, "content", None, None, 7)
            .unwrap();

        let provider_messages = store.project_running_turn_tool_messages("turn_1").unwrap();
        let provider_call = &provider_messages[0].tool_calls.as_ref().unwrap()[0];
        assert_eq!(provider_call.function.name, "invoke_tool");
        assert!(provider_call.function.arguments.contains("tool_name"));

        store.complete_turn("turn_1", "done", None).unwrap();
        let timeline = store.session_timeline(10).unwrap();
        assert_eq!(timeline[0].tools[0].name, "read_file");
        assert_eq!(timeline[0].tools[0].arguments, r#"{"path":"README.md"}"#);
    }

    /// 【时间线】【失败轮】错误摘要独立于正文：有正文时两者各归其位，
    /// 无正文时时间线正文清空，错误只出现在 error 字段。
    #[test]
    fn failed_turn_keeps_error_apart_from_partial_content() {
        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        // 1. 有部分正文的失败轮：正文保留，错误进 error 字段
        store.start_turn("turn_1", "做点事").unwrap();
        store
            .fail_turn("turn_1", "已经生成的部分正文", None, "上游请求超时")
            .unwrap();
        // 2. 无正文的失败轮：错误只出现在 error 字段，正文不再重复
        store.start_turn("turn_2", "再做点事").unwrap();
        store.fail_turn("turn_2", "", None, "供应商 500").unwrap();

        let timeline = store.session_timeline(10).unwrap();

        assert_eq!(timeline[0].status, "failed");
        assert_eq!(timeline[0].assistant.content, "已经生成的部分正文");
        assert_eq!(timeline[0].error.as_deref(), Some("上游请求超时"));
        assert_eq!(timeline[1].assistant.content, "");
        assert_eq!(timeline[1].error.as_deref(), Some("供应商 500"));
    }

    /// 【时间线】【模型切换】记录模型后可读回映射，未记录的轮次不在其中。
    #[test]
    fn records_turn_model_for_timeline_dividers() {
        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        store.start_turn("turn_1", "hello").unwrap();
        store.set_turn_model("turn_1", "big-pickle").unwrap();
        store.complete_turn("turn_1", "done", None).unwrap();
        store.start_turn("turn_2", "again").unwrap();
        // 空白模型不写入，轮次保持未记录状态
        store.set_turn_model("turn_2", "  ").unwrap();
        store.complete_turn("turn_2", "done", None).unwrap();

        let models = store.turn_models().unwrap();

        assert_eq!(models.get("turn_1").map(String::as_str), Some("big-pickle"));
        assert!(!models.contains_key("turn_2"));
        // 加载轮次同样携带模型，供其它读路径复用
        let turns = store.load_turns().unwrap();
        assert_eq!(turns[0].model.as_deref(), Some("big-pickle"));
        assert_eq!(turns[1].model, None);
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
        store
            .complete_turn("turn_external", "continued", None)
            .unwrap();

        let timeline = store.session_timeline(10).unwrap();

        assert!(timeline[0].automatic);
    }

    #[test]
    fn exposes_provider_injected_prefix_separately_from_visible_user_text() {
        let temp = tempfile::tempdir().unwrap();
        let store = StateStore::new(&test_paths(temp.path().to_path_buf())).unwrap();
        store.start_turn("turn_inject", "hello").unwrap();
        store
            .set_provider_user_content(
                "turn_inject",
                "<context-state>\n{\"kind\":\"runtime_change\"}\n</context-state>\n\nhello",
            )
            .unwrap();
        store.complete_turn("turn_inject", "ok", None).unwrap();

        let timeline = store.session_timeline(10).unwrap();

        assert_eq!(timeline[0].user.content, "hello");
        assert_eq!(
            timeline[0].injected_content.as_deref(),
            Some("<context-state>\n{\"kind\":\"runtime_change\"}\n</context-state>")
        );
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
        assert!(compaction.compacted_from_seq >= 1);
        assert!(compaction.compacted_to_seq >= compaction.compacted_from_seq);
    }
}
