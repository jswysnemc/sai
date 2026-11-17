use super::*;
use crate::cli::repl_exit_guard::{
    exit_choice, exit_question, fallback_notice, running_work, stop_work, ExitChoice, RunningWork,
};

/// 【退出确认】【入口】退出前检查后台工作；有工作时弹出选择，取消退出时恢复原有全屏视图。
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
    // 1. 【退出确认】【交互选择】记录进入前的全屏状态，取消或停止失败留在会话时恢复全屏
    if crate::question_tui::available(false) {
        let was_fullscreen = runtime.fullscreen_active();
        let choice = match ask_exit_choice(&work, runtime) {
            Ok(choice) => choice,
            Err(error) => {
                if was_fullscreen {
                    let _ = runtime.enter_fullscreen();
                }
                return Err(error);
            }
        };
        return finish_choice(choice, paths, config, state, runtime, &work, was_fullscreen).await;
    }
    // 2. 【退出确认】【文本回退】无法弹出选择时第一次提示，第二次停止后退出
    if *exit_armed {
        return finish_choice(
            ExitChoice::StopAndExit,
            paths,
            config,
            state,
            runtime,
            &work,
            runtime.fullscreen_active(),
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
    // 1. 【退出确认】【主屏交接】提问面板在主屏绘制，全屏视图暂时退出
    runtime.leave_fullscreen()?;
    let mut stdout = io::stdout();
    // 2. 【退出确认】【独占输入】启用临时输入守卫，避免与输入框事件竞争
    let mut guard = terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
    runtime.pause_for_permission_prompt()?;
    let response = crate::question_tui::ask(&exit_question(work))
        .unwrap_or_else(|error| crate::question::QuestionResponse::Unavailable(error.to_string()));
    // 3. 【退出确认】【恢复标记】恢复终端模式并标记受管区域待同步
    let _ = guard.finish(&mut stdout);
    runtime.mark_desynced();
    Ok(exit_choice(&response))
}

/// 【退出确认】【执行选择】按选择停止后台工作并决定是否退出，留在会话时按需恢复全屏。
///
/// 参数:
/// - `choice`: 用户选择
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `state`: 当前会话状态
/// - `runtime`: 终端运行期
/// - `work`: 仍在运行的工作
/// - `was_fullscreen`: 进入退出确认前是否处于全屏视图
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
    was_fullscreen: bool,
) -> Result<bool> {
    match choice {
        ExitChoice::Stay => {
            // 1. 【退出确认】【恢复全屏】取消退出时先恢复原有全屏画布再写入提示
            restore_session_screen(runtime, was_fullscreen)?;
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
            // 2. 【退出确认】【停止失败】留在会话里供用户查看原因，同时恢复原有全屏视图
            restore_session_screen(runtime, was_fullscreen)?;
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

/// 【退出确认】【视图恢复】当退出流程被取消或中止时，按进入前状态恢复全屏视图。
///
/// 参数:
/// - `runtime`: 终端运行期
/// - `was_fullscreen`: 进入退出确认前是否为全屏模式
///
/// 返回:
/// - 恢复结果
fn restore_session_screen(runtime: &mut ReplRuntime, was_fullscreen: bool) -> Result<()> {
    if was_fullscreen {
        runtime.enter_fullscreen()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::transcript::TranscriptRenderOptions;

    /// 【退出确认测试】【全屏恢复】取消退出后恢复进入前的全屏状态，内联模式保持内联。
    #[tokio::test]
    async fn stay_choice_restores_fullscreen_when_previously_active() {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        paths.create_dirs().unwrap();
        let config = AppConfig::default();
        let state = StateStore::new(&paths).unwrap();
        state.init_files().unwrap();
        let mut runtime = ReplRuntime::new(
            1000,
            TranscriptRenderOptions {
                reasoning_mode: render::ReasoningDisplayMode::Summary,
                tool_call_mode: render::ToolCallDisplayMode::Summary,
            },
        );
        runtime.enter_fullscreen().unwrap();
        assert!(runtime.fullscreen_active());
        runtime.leave_fullscreen().unwrap();
        assert!(!runtime.fullscreen_active());

        let work = RunningWork::default();
        let exited = finish_choice(
            ExitChoice::Stay,
            &paths,
            &config,
            &state,
            &mut runtime,
            &work,
            true,
        )
        .await
        .unwrap();
        assert!(!exited);
        assert!(runtime.fullscreen_active());

        runtime.leave_fullscreen().unwrap();
        let exited_inline = finish_choice(
            ExitChoice::Stay,
            &paths,
            &config,
            &state,
            &mut runtime,
            &work,
            false,
        )
        .await
        .unwrap();
        assert!(!exited_inline);
        assert!(!runtime.fullscreen_active());
    }
}
