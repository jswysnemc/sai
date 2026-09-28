use super::*;

/// 有后台工作时先提示，第二次退出才真正离开。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `state`: 当前会话状态
/// - `runtime`: 终端运行期，用于写出提示
/// - `exit_armed`: 是否已经提示过
///
/// 返回:
/// - 可以退出时返回 true
pub(super) fn confirm_repl_exit(
    paths: &SaiPaths,
    state: &crate::state::StateStore,
    runtime: &mut ReplRuntime,
    exit_armed: &mut bool,
) -> Result<bool> {
    if *exit_armed {
        return Ok(true);
    }
    let Some(notice) = super::repl_exit_guard::background_exit_notice(
        paths,
        state.session_id(),
        state.state_dir(),
    ) else {
        return Ok(true);
    };
    runtime.record_meta(notice)?;
    *exit_armed = true;
    Ok(false)
}
