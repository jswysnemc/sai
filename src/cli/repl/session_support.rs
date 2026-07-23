use crate::agent::{Agent, AgentMode};
use crate::cli::build_repl_tool_registry;
#[cfg(test)]
use crate::cli::build_repl_tool_registry_for_session;
use crate::cli::providers::apply_thinking_override;
use crate::cli::repl_runtime::ReplRuntime;
use crate::cli::repl_text::strip_terminal_control_sequences;
use crate::cli::repl_tool_warmup::ReplToolWarmup;
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use crate::render::command_result_streams;
use crate::state::{SessionTimelineTurn, StateStore};
use anyhow::Result;

const REPL_HISTORY_TURN_LIMIT: usize = 50;

/// 读取当前会话最近的持久化轮次并渲染到 TUI。
///
/// 参数:
/// - `runtime`: 当前 TUI 运行期
/// - `state`: 当前会话状态存储
///
/// 返回:
/// - 历史读取与渲染结果
pub(super) fn record_repl_history(runtime: &mut ReplRuntime, state: &StateStore) -> Result<()> {
    let mut timeline = state.session_timeline_with_compaction(REPL_HISTORY_TURN_LIMIT)?;
    restore_history_command_outputs(&mut timeline.turns, |result_ref| {
        state.read_tool_result_ref(result_ref)
    });
    runtime.record_history_with_compaction(&timeline.turns, timeline.compaction.as_ref())
}

/// 从安全的结果引用恢复历史命令完整输出。
///
/// 参数:
/// - `turns`: 待补全的历史轮次
/// - `read_result_ref`: 读取会话内结果引用的函数
///
/// 返回:
/// - 无；引用异常时直接写入可读提示
fn restore_history_command_outputs<F>(turns: &mut [SessionTimelineTurn], mut read_result_ref: F)
where
    F: FnMut(&str) -> Result<String>,
{
    for tool in turns
        .iter_mut()
        .flat_map(|turn| turn.tools.iter_mut())
        .filter(|tool| tool.name == "run_command")
    {
        let Some(result_ref) = tool.result_ref.as_deref() else {
            let preview_is_truncated = tool
                .original_chars
                .is_some_and(|original| original > tool.output.chars().count());
            if preview_is_truncated && command_result_streams(&tool.output).is_none() {
                tool.output = unavailable_command_output(None, false);
            }
            continue;
        };

        tool.output = match read_result_ref(result_ref) {
            Ok(output) if command_result_streams(&output).is_some() => output,
            Ok(_) => unavailable_command_output(Some(result_ref), true),
            Err(_) => unavailable_command_output(Some(result_ref), false),
        };
    }
}

/// 生成人类可读的历史命令输出缺失提示。
///
/// 参数:
/// - `result_ref`: 可选的持久化结果引用
/// - `invalid_content`: 引用内容是否损坏
///
/// 返回:
/// - 不包含协议 JSON 的提示文本
fn unavailable_command_output(result_ref: Option<&str>, invalid_content: bool) -> String {
    let reason = if invalid_content {
        t("stored command result is invalid", "保存的命令结果内容无效")
    } else {
        t(
            "stored command result is missing or unavailable",
            "保存的命令结果缺失或无法读取",
        )
    };
    match result_ref {
        Some(result_ref) => format!(
            "{}: {reason} ({result_ref})",
            t("Command output unavailable", "历史命令输出不可用")
        ),
        None => format!(
            "{}: {reason}",
            t("Command output unavailable", "历史命令输出不可用")
        ),
    }
}

/// 将后台发现完成的 MCP 工具无阻塞合并到当前 Agent。
///
/// 参数:
/// - `warmup`: MCP 工具预热任务
/// - `agent`: 当前复用的 Agent
/// - `mode`: 当前输入选择的模式
/// - `runtime`: TUI 运行期
///
/// 返回:
/// - 合并或错误展示结果
pub(super) fn apply_ready_tool_registry(
    warmup: &mut ReplToolWarmup,
    agent: &mut Agent,
    mode: AgentMode,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    let Some(result) = warmup.take_ready() else {
        return Ok(());
    };
    match result {
        Ok((warmup_mode, registry)) if warmup_mode == mode => agent.replace_tools(registry),
        Ok(_) => {}
        Err(error) => runtime.record_meta(format!(
            "{}: {error}",
            t("MCP tool discovery failed", "MCP 工具发现失败")
        ))?,
    }
    Ok(())
}

/// 返回欢迎面板中展示的当前模型名称。
///
/// 参数:
/// - `config`: 当前应用配置
///
/// 返回:
/// - 当前模型名称，未配置时返回占位符
pub(super) fn repl_welcome_model(config: &AppConfig) -> String {
    config
        .provider(None)
        .ok()
        .map(|provider| provider.default_model.trim().to_string())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| "-".to_string())
}

/// 重载配置与客户端，并同步到复用中的 Agent。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 可变配置
/// - `client`: 可变 LLM 客户端
/// - `agent`: 复用中的 Agent
/// - `mode`: 当前模式
/// - `thinking_override`: 命令行思考等级覆盖
///
/// 返回:
/// - 重载结果
pub(super) fn reload_repl_agent(
    paths: &SaiPaths,
    config: &mut AppConfig,
    client: &mut OpenAiCompatibleClient,
    agent: &mut Agent,
    mode: AgentMode,
    thinking_override: Option<&str>,
) -> Result<()> {
    *config = crate::config::apply_agent_override(
        AppConfig::load(paths)?,
        None,
        crate::config::AgentSurface::Tui,
    )?;
    apply_thinking_override(config, thinking_override)?;
    *client = OpenAiCompatibleClient::from_config(config, paths)?;
    let registry = build_repl_tool_registry(config, paths, mode)?;
    agent.reload(config.clone(), client.clone(), registry, mode)?;
    Ok(())
}

/// 读取可供 REPL 浏览的用户输入历史。
///
/// 参数:
/// - `state`: 当前会话状态存储
///
/// 返回:
/// - 已清理终端控制序列的用户输入列表
pub(in crate::cli) fn load_repl_input_history(state: &StateStore) -> Result<Vec<String>> {
    Ok(state
        .load_conversation()?
        .into_iter()
        .filter(|entry| {
            entry.role == "user"
                && !entry.content.trim().is_empty()
                && !crate::goal::is_continuation_input(&entry.content)
        })
        .map(|entry| strip_terminal_control_sequences(&entry.content))
        .filter(|content| !content.trim().is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{TimelineMessage, TimelineToolEntry};

    /// 构造包含历史命令结果的最小轮次。
    ///
    /// 参数:
    /// - `output`: 持久化预览
    /// - `result_ref`: 可选完整结果引用
    /// - `original_chars`: 原始结果字符数
    ///
    /// 返回:
    /// - 可用于恢复测试的历史轮次
    fn command_turn(
        output: &str,
        result_ref: Option<&str>,
        original_chars: usize,
    ) -> SessionTimelineTurn {
        SessionTimelineTurn {
            turn_id: "turn-1".to_string(),
            seq: 1,
            status: "completed".to_string(),
            user: TimelineMessage {
                timestamp: String::new(),
                content: "执行命令".to_string(),
                reasoning: None,
                image_urls: Vec::new(),
            },
            assistant: TimelineMessage {
                timestamp: String::new(),
                content: String::new(),
                reasoning: None,
                image_urls: Vec::new(),
            },
            tools: vec![TimelineToolEntry {
                id: "call-1".to_string(),
                name: "run_command".to_string(),
                arguments: r#"{"command":"printf long"}"#.to_string(),
                status: "completed".to_string(),
                output: output.to_string(),
                ok: Some(true),
                error: None,
                result_ref: result_ref.map(str::to_string),
                original_chars: Some(original_chars),
                created_at: String::new(),
                completed_at: None,
                permission: None,
            }],
            automatic: false,
        }
    }

    /// 验证 REPL 审计模式构造的工具注册表绑定了权限配置。
    #[test]
    fn audited_repl_registry_intercepts_tools_before_execution() {
        let paths = SaiPaths::new().unwrap();
        let config = AppConfig::load_or_default(&paths).unwrap();
        let state_dir = tempfile::tempdir().unwrap();
        let registry = build_repl_tool_registry_for_session(
            &config,
            &paths,
            AgentMode::Audited,
            "test-session",
            state_dir.path(),
        )
        .unwrap();

        assert!(registry
            .requires_permission("edit_file", r#"{"path":"src/main.rs","content":"x"}"#)
            .unwrap());
        let sensitive_read = if cfg!(windows) {
            r#"{"path":"C:\\Windows\\System32\\drivers\\etc\\hosts"}"#
        } else {
            r#"{"path":"/etc/hosts"}"#
        };
        assert!(registry
            .requires_permission("read_file", sensitive_read)
            .unwrap());
        assert!(!registry
            .requires_permission("read_file", r#"{"path":"src/main.rs"}"#)
            .unwrap());
        assert!(!registry
            .requires_permission("todo", r#"{"action":"add","text":"检查"}"#)
            .unwrap());
    }

    /// 历史命令通过 result_ref 恢复完整协议结果，供渲染层提取输出流。
    #[test]
    fn restores_full_historical_command_result_from_reference() {
        let full = r#"{"success":true,"exit_code":0,"stdout":"完整输出","stderr":""}"#;
        let mut turns = vec![command_turn(
            r#"{"success":true,"stdout":"截断"#,
            Some("tool-results/call-1.txt"),
            full.chars().count(),
        )];

        restore_history_command_outputs(&mut turns, |_| Ok(full.to_string()));

        assert_eq!(turns[0].tools[0].output, full);
    }

    /// 引用缺失时显示可读提示，不回退展示被截断的协议 JSON。
    #[test]
    fn missing_historical_command_result_does_not_render_protocol_json() {
        let mut turns = vec![command_turn(
            r#"{"success":true,"stdout":"截断"#,
            Some("tool-results/missing.txt"),
            20_000,
        )];

        restore_history_command_outputs(&mut turns, |_| anyhow::bail!("missing"));

        let output = &turns[0].tools[0].output;
        assert!(!output.trim_start().starts_with('{'));
        assert!(output.contains("tool-results/missing.txt"));
    }

    /// 缺少 result_ref 的截断预览同样不会泄露协议 JSON。
    #[test]
    fn truncated_historical_command_without_reference_uses_readable_message() {
        let mut turns = vec![command_turn(
            r#"{"success":true,"stdout":"截断"#,
            None,
            20_000,
        )];

        restore_history_command_outputs(&mut turns, |_| unreachable!());

        assert!(!turns[0].tools[0].output.trim_start().starts_with('{'));
    }
}
