use super::config_options::AcpConfigOptions;
use super::event_bridge::BridgedUpdate;
use crate::agent_engine::EventSender;

/// 一次 ACP prompt 聚合后的结果。
#[derive(Default)]
pub(super) struct PromptOutcome {
    pub(super) content: String,
    pub(super) reasoning: String,
    pub(super) usage: Option<crate::llm::Usage>,
    pub(super) compaction_started: bool,
    pub(super) compaction_applied: Option<bool>,
}

/// 把一条 ACP 更新合并到本轮聚合状态并发布运行时信息。
///
/// 参数:
/// - `engine_name`: 外部内核稳定名称
/// - `config_options`: 当前会话配置项存储
/// - `bridged`: 已翻译的 ACP 更新
/// - `outcome`: 本轮正文、推理、用量和压缩状态
/// - `events`: Sai 事件发送端
///
/// 返回:
/// - 无
pub(super) fn apply_bridged_update(
    engine_name: &str,
    config_options: &mut AcpConfigOptions,
    bridged: BridgedUpdate,
    outcome: &mut PromptOutcome,
    events: &EventSender,
) {
    // 【ACP】【轮次状态】1. 保存 agent 动态公布的配置、模式和斜杠命令
    config_options.replace(bridged.config_options.clone());
    if let Some(options) = &bridged.config_options {
        super::runtime_state::update_config_options(engine_name, options);
    }
    if let Some(mode) = &bridged.current_mode {
        super::runtime_state::update_current_mode(engine_name, mode);
    }
    if let Some(commands) = &bridged.available_commands {
        super::runtime_state::update_available_commands(engine_name, commands);
    }
    // 【ACP】【轮次状态】2. 聚合本轮内容、用量和上下文压缩观察结果
    if bridged.usage.is_some() {
        outcome.usage = bridged.usage.clone();
    }
    outcome.compaction_started |= bridged.compaction_started;
    if bridged.compaction_applied.is_some() {
        outcome.compaction_applied = bridged.compaction_applied;
    }
    outcome.content.push_str(&bridged.content_delta);
    outcome.reasoning.push_str(&bridged.reasoning_delta);
    // 【ACP】【轮次状态】3. 按协议顺序把翻译后的事件交给运行界面
    for event in bridged.events {
        let _ = events.send(event);
    }
}
