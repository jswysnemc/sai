use super::*;
use crate::cli::repl_exit_guard::{
    exit_choice, exit_question, fallback_notice, running_work, stop_work, ExitChoice, RunningWork,
};

/// 【退出确认】【入口】退出前检查后台工作；有工作时弹出选择，按选择停止或保留后再退出。
///
/// 终端无法弹出选择时退回两步确认：第一次只提示，再执行一次退出才停止工作并关闭。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置，停止后台命令时使用
/// - `state`: 当前会话状态
/// - `runtime`: 终端运行期
/// - `exit_armed`: 文本两步确认是否已经提示过
///
/// 返回:
/// - 可以退出时返回 true
pub(super) async fn confirm_repl_exit(
    paths: &SaiPaths,
    config: &AppConfig,
    state: &crate::state::StateStore,
    runtime: &mut ReplRuntime,
    exit_armed: &mut bool,
) -> Result<bool> {
    let work = running_work(paths, state.session_id(), state.state_dir());
    if work.is_empty() {
        return Ok(true);
    }
    // 1. 交互终端：弹出三选一
    if crate::question_tui::available(false) {
        let choice = ask_exit_choice(&work, runtime)?;
        return finish_choice(choice, paths, config, state, runtime, &work).await;
    }
    // 2. 无法弹出选择：第一次提示，第二次停止后退出
    if *exit_armed {
        return finish_choice(
            ExitChoice::StopAndExit,
            paths,
            config,
            state,
            runtime,
            &work,
        )
        .await;
    }
    runtime.record_meta(fallback_notice(&work))?;
    *exit_armed = true;
    Ok(false)
}

/// 【退出确认】【弹出选择】在主屏光标处显示退出确认并读取选择。
///
/// 参数:
/// - `work`: 仍在运行的工作
/// - `runtime`: 终端运行期
///
/// 返回:
/// - 用户选择；取消或出错时留在会话
fn ask_exit_choice(work: &RunningWork, runtime: &mut ReplRuntime) -> Result<ExitChoice> {
    // 提问面板在主屏绘制，全屏视图必须先退出
    runtime.leave_fullscreen()?;
    let mut stdout = io::stdout();
    // 1. 独占 raw 输入，避免与输入框事件竞争
    let mut guard = terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
    runtime.pause_for_permission_prompt()?;
    let response = crate::question_tui::ask(&exit_question(work))
        .unwrap_or_else(|error| crate::question::QuestionResponse::Unavailable(error.to_string()));
    // 2. 恢复终端模式；面板直接写过终端，受管区域在下次同步前重启
    let _ = guard.finish(&mut stdout);
    runtime.mark_desynced();
    Ok(exit_choice(&response))
}

/// 【退出确认】【执行选择】按选择停止后台工作并决定是否退出。
///
/// 参数:
/// - `choice`: 用户选择
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `state`: 当前会话状态
/// - `runtime`: 终端运行期
/// - `work`: 仍在运行的工作
///
/// 返回:
/// - 可以退出时返回 true
async fn finish_choice(
    choice: ExitChoice,
    paths: &SaiPaths,
    config: &AppConfig,
    state: &crate::state::StateStore,
    runtime: &mut ReplRuntime,
    work: &RunningWork,
) -> Result<bool> {
    match choice {
        ExitChoice::Stay => {
            runtime.record_meta(
                crate::i18n::text(
                    "Exit cancelled; background work keeps running.",
                    "已取消退出，后台工作继续运行。",
                )
                .to_string(),
            )?;
            Ok(false)
        }
        ExitChoice::LeaveRunning => Ok(true),
        ExitChoice::StopAndExit => {
            let failures = stop_work(paths, config, state.state_dir(), work).await;
            if failures.is_empty() {
                return Ok(true);
            }
            // 停止失败时留在会话里，让用户看到原因再决定
            let detail = failures.join("; ");
            runtime.record_meta(if crate::i18n::is_zh() {
                format!("部分后台工作停止失败，未退出：{detail}")
            } else {
                format!(
                    "Some background work could not be stopped; staying in the session: {detail}"
                )
            })?;
            Ok(false)
        }
    }
}
