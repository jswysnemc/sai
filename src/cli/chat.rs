use super::*;

/// 保存 shell hook 在命令执行前截获的命令。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `shell_name`: 当前 shell 名称
/// - `message`: 拦截到的自然语言文本
/// - `clipb`: 是否读取剪贴板
/// - `web_search`: 是否启用网络搜索模型
///
/// 返回:
/// - 执行是否成功
pub(super) async fn run_shell_intercept(
    paths: &SaiPaths,
    shell_name: &str,
    message: String,
    clipb: bool,
    web_search: bool,
) -> Result<()> {
    if !matches!(shell_name, "fish" | "bash" | "zsh" | "powershell") {
        bail!("{}: {shell_name}", t("unsupported shell", "不支持的 shell"));
    }
    if message.is_empty() {
        bail!(
            "{}",
            t("empty intercepted shell command", "拦截到的 shell 命令为空")
        );
    }
    crate::shell::intercept_store::store(paths, shell_name, &message, clipb, web_search)?;
    Ok(())
}

/// 使用最近一次 shell 命令开启解释对话。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `instruction`: 用户补充要求
/// - `clipb`: 是否读取剪贴板
/// - `web_search`: 是否使用网络搜索
/// - `mode`: CLI 当前权限模式
/// - `thinking_override`: 可选思考等级
///
/// 返回:
/// - 对话执行结果
pub(super) async fn run_stored_shell_explanation(
    paths: &SaiPaths,
    instruction: String,
    clipb: bool,
    web_search: bool,
    mode: AgentMode,
    thinking_override: Option<String>,
) -> Result<()> {
    let record = crate::shell::intercept_store::load(paths)?.ok_or_else(|| {
        anyhow::anyhow!(t(
            "no intercepted shell command is available",
            "没有可解释的 shell 命令记录"
        ))
    })?;
    run_chat_with_options(
        paths,
        ChatRunOptions {
            message: crate::shell::intercept_store::prompt(&record, &instruction),
            source: crate::runner::SubmissionSource::Command,
            show_reasoning: None,
            plain: false,
            mode,
            clipb: clipb || record.clipb,
            web_search: web_search || record.web_search,
            thinking_override,
            show_final_summary: true,
        },
    )
    .await
}

/// 清理当前终端标准输入中残留的按键内容。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 无
#[cfg(unix)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn drain_stdin() {
    use std::os::fd::AsRawFd;

    let stdin = io::stdin();
    if !stdin.is_terminal() {
        return;
    }
    let fd = stdin.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return;
    }
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return;
    }

    let mut handle = stdin.lock();
    let mut buffer = [0_u8; 4096];
    loop {
        match handle.read(&mut buffer) {
            Ok(0) => break,
            Ok(_) => continue,
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => break,
            Err(_) => break,
        }
    }

    let _ = unsafe { libc::fcntl(fd, libc::F_SETFL, flags) };
}

/// 清理 Windows 控制台标准输入中残留的按键事件。
///
/// 返回:
/// - 无
#[cfg(windows)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn drain_stdin() {
    if !io::stdin().is_terminal() {
        return;
    }
    // 1. 只读取已经进入控制台队列的事件，避免等待新输入
    while event::poll(Duration::ZERO).unwrap_or(false) {
        if event::read().is_err() {
            break;
        }
    }
}

/// 非 Unix 和 Windows 平台不执行终端输入清理。
///
/// 返回:
/// - 无
#[cfg(not(any(unix, windows)))]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn drain_stdin() {}

/// 把管道 stdin 的内容并入消息文本。
///
/// 参数:
/// - `message`: 命令行携带的消息，可为空
///
/// 返回:
/// - 合并后的消息；stdin 为终端时原样返回
fn merge_piped_stdin_message(message: String) -> Result<String> {
    if io::stdin().is_terminal() {
        return Ok(message);
    }
    let mut piped = String::new();
    io::stdin().read_to_string(&mut piped)?;
    let piped = piped.trim();
    if piped.is_empty() {
        return Ok(message);
    }
    if message.is_empty() {
        return Ok(piped.to_string());
    }
    // 命令行消息作为指令，管道内容作为其处理对象附在其后
    Ok(format!("{message}\n\n{piped}"))
}

/// 校验交互 REPL 所需的终端环境。
///
/// 返回:
/// - stdin 或 stdout 不是终端时返回可操作错误
pub(super) fn ensure_interactive_terminal_for_repl() -> Result<()> {
    if !io::stdin().is_terminal() {
        bail!(
            "{}",
            t(
                "stdin is not a terminal and carried no input; pass a message: sai \"question\" or echo \"question\" | sai",
                "stdin 不是终端且没有输入内容；请直接传入消息：sai \"问题\" 或 echo \"问题\" | sai"
            )
        );
    }
    if !io::stdout().is_terminal() {
        bail!(
            "{}",
            t(
                "interactive mode needs a terminal on stdout; pass a message for one-shot output: sai \"question\"",
                "交互模式需要 stdout 是终端；重定向输出请改用一次性调用：sai \"问题\""
            )
        );
    }
    Ok(())
}

/// 单次命令聊天执行选项。
pub(super) struct ChatRunOptions {
    pub(super) message: String,
    pub(super) source: crate::runner::SubmissionSource,
    pub(super) show_reasoning: Option<bool>,
    pub(super) plain: bool,
    pub(super) mode: AgentMode,
    pub(super) clipb: bool,
    pub(super) web_search: bool,
    pub(super) thinking_override: Option<String>,
    pub(super) show_final_summary: bool,
}

/// 执行一次非交互聊天请求。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `options`: 聊天执行选项
///
/// 返回:
/// - 执行是否成功
pub(super) async fn run_chat_with_options(paths: &SaiPaths, options: ChatRunOptions) -> Result<()> {
    let ChatRunOptions {
        message,
        source,
        show_reasoning,
        plain,
        mode,
        clipb,
        web_search,
        thinking_override,
        show_final_summary,
    } = options;
    // 1. 管道输入读作消息内容：echo "问题" | sai 与 sai "指令" < 文件 均可用
    let message = merge_piped_stdin_message(message)?;
    if message.is_empty() && !clipb && !web_search {
        ensure_interactive_terminal_for_repl()?;
        return run_repl(paths, mode, thinking_override).await;
    }
    if message.is_empty() && web_search && !clipb {
        bail!(
            "{}",
            t(
                "web search needs a message, e.g. sai -w \"latest rust release\"",
                "网络搜索需要同时提供消息内容，例如 sai -w \"rust 最新版本\""
            )
        );
    }
    AppConfig::init_files(paths)?;
    let mut config = AppConfig::load_or_default(paths)?;
    config = crate::config::apply_agent_override(config, None, crate::config::AgentSurface::Cli)?;
    apply_thinking_override(&mut config, thinking_override.as_deref())?;
    if web_search {
        let choice =
            config.select_active_provider_model_with_tag(crate::config::MODEL_TAG_WEB_SEARCH)?;
        println!(
            "{}: {}",
            t("web search model", "网络搜索模型"),
            choice.label()
        );
    }
    config.active_context_window_tokens()?;
    let chat_input = prepare_clipboard_chat_input(message, clipb)?;
    let reasoning_mode = if show_reasoning == Some(false) {
        render::ReasoningDisplayMode::Hidden
    } else {
        render::ReasoningDisplayMode::from_config(&config.display.reasoning)
    };
    let tool_call_mode = if plain {
        render::ToolCallDisplayMode::Hidden
    } else {
        render::ToolCallDisplayMode::from_config(&config.display.tool_calls)
    };
    let render_options = stream_render_options(&config);
    let render_policy = crate::runner::RenderPolicy::new(
        plain,
        reasoning_mode,
        tool_call_mode,
        render_options.clone(),
    );
    let user_input = match chat_input.image_url {
        Some(image_url) => crate::runner::UserInputSubmission::new(chat_input.message, mode)
            .with_image_url(image_url),
        None => crate::runner::UserInputSubmission::new(chat_input.message, mode),
    };
    let submission = crate::runner::RunnerSubmission::user_input(source, user_input)
        .with_render_policy(render_policy)
        .with_final_summary(show_final_summary && !plain);
    let mut renderer =
        render::StreamRenderer::new(reasoning_mode, tool_call_mode, plain, render_options);
    // 一次性模式没有 raw mode，Ctrl+C 由默认信号处理直接终止进程，
    // 渲染器 Drop 不执行会把隐藏的光标留在终端；接管后先恢复再退出
    tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            super::terminal_restore::emergency_restore();
            std::process::exit(130);
        }
    });
    renderer.start_waiting()?;
    let mut runner_output = crate::runner::RunnerOutput::default();
    let mut final_summary = None;
    let result = {
        let mut sink = |event: crate::runner::RunnerEvent| {
            match &event {
                crate::runner::RunnerEvent::WaitingExternal => {
                    renderer.start_waiting_external()?;
                }
                crate::runner::RunnerEvent::Agent(agent_event) => {
                    handle_agent_event(&mut renderer, agent_event.clone())?;
                }
                crate::runner::RunnerEvent::FinalSummary(snapshot) => {
                    final_summary = Some(snapshot.clone());
                }
                _ => {}
            }
            runner_output.push_event(event);
            Ok(())
        };
        crate::runner::SessionRunner::new(paths)
            .with_config(config)
            .run_submission(submission, &mut sink)
            .await
    };
    renderer.finish()?;
    if let Err(err) = result {
        // 错误已完整展示给用户，向上返回静默退出标记避免主入口再打印一遍
        render::write_chat_error(&err, plain)?;
        return Err(anyhow::Error::new(super::SilentExit { code: 1 }));
    }
    if let Some(snapshot) = final_summary {
        render::print_session_summary(&snapshot)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn stored_shell_prompt_keeps_user_instruction() {
        let record = crate::shell::intercept_store::StoredShellCommand {
            shell: "zsh".to_string(),
            command: "source missing.zsh".to_string(),
            clipb: false,
            web_search: false,
        };

        let prompt = crate::shell::intercept_store::prompt(&record, "explain this command");
        assert!(prompt.contains("source missing.zsh"));
        assert!(prompt.contains("explain this command"));
    }
}
