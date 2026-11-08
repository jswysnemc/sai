use super::*;

/// 【终端】【会话模式】处理权限模式命令和 `/code-model`。
///
/// 参数: `input` 为提交文本，`mode` 为当前权限模式，`runtime` 为终端界面
/// 返回: 命中模式命令并完成切换时返回 true
pub(super) fn handle(
    input: &str,
    mode: &mut AgentMode,
    runtime: &mut ReplRuntime,
    state_dir: &std::path::Path,
) -> Result<bool> {
    let Some(next) = mode.command(input) else {
        return Ok(false);
    };
    if next.permission() == crate::agent::PermissionMode::Plan
        && mode.permission() != crate::agent::PermissionMode::Plan
    {
        crate::plan::store::begin_sync(state_dir, *mode)?;
    }
    *mode = next;
    runtime.record_meta(format!("{}: {}", t("mode", "模式"), mode.label()))?;
    Ok(true)
}
