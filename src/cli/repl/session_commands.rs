use super::*;

/// 【上下文】【命令分类】识别可以在当前画布中显示的上下文命令及别名。
/// 参数：input 为完整用户命令；返回：命令是否使用内联上下文面板。
pub(super) fn uses_context_panel(input: &str) -> bool {
    matches!(
        crate::control_commands::parse_control_command(
            input,
            crate::control_commands::ControlSurface::Repl
        ),
        Ok(Some(
            crate::control_commands::ControlCommand::Context { .. }
        ))
    )
}

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
    // 1. 【上下文】【策略编辑】卡片接替输入区域，退出后恢复当前视图
    if update == Some(crate::control_commands::ContextPolicyUpdate::Edit) {
        runtime.record_user(mode, input.to_string(), false)?;
        let result = runtime.edit_context_policy(paths);
        return runtime.record_meta(result.unwrap_or_else(|error| error.to_string()));
    }
    // 2. 【上下文】【浮层入口】只读查询不写入会话记录，关闭或报错后恢复底层界面
    match crate::control_commands::context_overview(paths, mode, update) {
        Ok(view) => runtime.show_context_panel(&view),
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

#[cfg(test)]
mod tests {
    use super::uses_context_panel;

    /// 【上下文】【命令分类回归】编辑、查看、恢复及别名均保留当前视图，其他命令仍独占画布。
    /// 参数：无；返回：无，分类不符时断言失败。
    #[test]
    fn context_commands_preserve_the_active_surface() {
        for input in [
            "/context",
            "/context edit",
            "/content edit",
            "/context reset",
            "/context 85% 8k",
            "/CONTEXT ",
        ] {
            assert!(uses_context_panel(input), "{input}");
        }
        for input in ["/config", "/help", "/context invalid", "ordinary text"] {
            assert!(!uses_context_panel(input), "{input}");
        }
    }
}
