use super::*;

/// 【REPL】【上下文命令】处理 `/context`：查看上下文占用，或打开压缩策略编辑界面。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `mode`: 当前权限模式
/// - `update`: 上下文策略更新请求
/// - `input`: 用户原始输入，用于回显
/// - `runtime`: 终端运行期
///
/// 返回:
/// - 处理结果
pub(super) fn handle_context(
    paths: &SaiPaths,
    mode: AgentMode,
    update: Option<crate::control_commands::ContextPolicyUpdate>,
    input: &str,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    runtime.record_user(mode, input.to_string(), false)?;
    // 1. 编辑压缩策略：打开独立配置界面，返回后整屏重绘
    if update == Some(crate::control_commands::ContextPolicyUpdate::Edit) {
        let result = crate::config_tui::compaction::run_session(paths);
        runtime.redraw()?;
        return runtime.record_meta(result.unwrap_or_else(|error| error.to_string()));
    }
    // 2. 其余情况输出上下文占用说明
    match crate::control_commands::context_info_for_mode_with_update(paths, mode, update) {
        Ok(info) => runtime.record_meta(info),
        Err(err) => runtime.record_meta(err.to_string()),
    }
}

/// 【REPL】【子智能体留言】把用户留言直接投递到子智能体消息队列，不经过主 Agent 轮次。
///
/// 参数:
/// - `state`: 当前会话状态
/// - `runtime`: 终端运行期
/// - `target`: 指定的子智能体，缺省时使用正在查看的子智能体
/// - `message`: 留言内容
///
/// 返回:
/// - 处理结果
pub(super) fn deliver_subagent_message(
    state: &StateStore,
    runtime: &mut ReplRuntime,
    target: Option<&str>,
    message: &str,
) -> Result<()> {
    let owner_key = state.state_dir().display().to_string();
    let viewing = runtime.viewing_subagent_id();
    match subagent_commands::deliver_subagent_message(
        &owner_key,
        target,
        message,
        viewing.as_deref(),
    ) {
        Ok(notice) => runtime.record_meta(notice),
        Err(error) => runtime.record_meta(error.to_string()),
    }
}
