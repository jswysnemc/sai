use super::*;

/// 执行会话压缩，并在等待期间处理终端刷新和取消按键。
///
/// 参数: `paths` 为项目路径，`config` 为配置，`mode` 为权限模式，`agent` 为会话，`runtime` 为终端状态
/// 返回: 压缩执行与终端恢复结果
pub(super) async fn run_compaction(
    paths: &SaiPaths,
    config: &AppConfig,
    mode: AgentMode,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    let submission = crate::runner::RunnerSubmission::control(
        crate::runner::SubmissionSource::Repl,
        mode,
        crate::runner::ControlSubmission::new(crate::control_commands::ControlCommand::Compact),
    );
    let result = {
        let runner = crate::runner::SessionRunner::new(paths).with_config(config.clone());
        let runtime = std::cell::RefCell::new(&mut *runtime);
        let mut sink =
            |event: crate::runner::RunnerEvent| runtime.borrow_mut().record_runner_event(&event);
        let compact = runner.run_submission_with_agent(submission, agent, &mut sink);
        tokio::pin!(compact);
        let mut resize_tick = tokio::time::interval(Duration::from_millis(25));
        resize_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // 用 raw 模式轮询按键取消，而不是 tokio 的 ctrl_c：
        // 后者首次 poll 即永久接管 SIGINT，会让 cooked 段的
        // Ctrl+C 行为在使用 /compact 前后不一致
        let mut compact_guard =
            terminal_restore::TerminalInputGuard::enable(&mut io::stdout(), false)?;
        let compact_result = loop {
            tokio::select! {
                result = &mut compact => break result.map(|_| ()),
                _ = resize_tick.tick() => {
                    let mut runtime_ref = runtime.borrow_mut();
                    process_stream_tick(&mut *runtime_ref)?;
                    drop(runtime_ref);
                    if poll_compact_cancel()? {
                        runtime.borrow_mut().record_meta(
                            t("compaction cancelled", "已取消压缩")
                                .to_string(),
                        )?;
                        break Ok(());
                    }
                }
            }
        };
        let guard_result = compact_guard.finish(&mut io::stdout());
        compact_result.and(guard_result)
    };
    runtime.finish_stream()?;
    result?;
    Ok(())
}

/// 轮询 /compact 执行期间的取消按键。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 收到 Ctrl+C 或 Esc 时返回 true
fn poll_compact_cancel() -> Result<bool> {
    while event::poll(Duration::ZERO)? {
        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Release {
                continue;
            }
            let ctrl_c = matches!(key.code, KeyCode::Char('c'))
                && key.modifiers.contains(KeyModifiers::CONTROL);
            if ctrl_c || matches!(key.code, KeyCode::Esc) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
