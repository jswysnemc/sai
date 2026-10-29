use super::*;

/// 【终端】【启动展示】初始化终端、恢复历史并提前绘制输入区。
/// 参数: `config` 为应用配置，`paths` 为存储路径，`state` 为会话，`mode` 为权限模式
/// 返回: 已应用显示偏好的终端运行期。
pub(super) fn initialize_runtime(
    config: &AppConfig,
    paths: &SaiPaths,
    state: &StateStore,
    mode: AgentMode,
) -> Result<ReplRuntime> {
    // 1. 【终端】【启动展示】先查询颜色和光标，避免与键盘输入争用终端
    render::terminal_palette::initialize_terminal_palette();
    if crossterm::cursor::position()
        .map(|(col, _)| col != 0)
        .unwrap_or(false)
    {
        println!();
    }
    let options = render::transcript::TranscriptRenderOptions {
        reasoning_mode: render::ReasoningDisplayMode::from_config(&config.display.reasoning),
        tool_call_mode: render::ToolCallDisplayMode::from_config(&config.display.tool_calls),
    };
    let mut runtime = ReplRuntime::new(config.display.repl_transcript_row_cap, options);
    render::configure_display_rendering(&config.display);
    if render::tui_mode::prefers_fullscreen() {
        runtime.toggle_fullscreen()?;
    }
    runtime.set_paste_image_key(config.input.paste_image_key);
    // 2. 【终端】【启动展示】欢迎信息与历史保存源数据，样式切换时重新渲染
    runtime.record_welcome(
        env!("CARGO_PKG_VERSION").to_string(),
        repl_welcome_model(config),
        crate::runtime_cwd::current_dir()
            .map(|path| crate::cli::repl_chrome::compress_home_prefix(&path.display().to_string()))
            .unwrap_or_else(|_| "~".to_string()),
        sandbox_commands::welcome_mode_label(mode),
    )?;
    if let Some(notice) = render::engine_notice(&config.agent) {
        runtime.record_meta(notice)?;
    }
    record_repl_history(&mut runtime, state)?;
    // 3. 【终端】【启动展示】重量级初始化前先呈现输入框
    let chrome = ReplChrome::from_runtime(config, paths, state, mode);
    runtime.update_composer(&chrome, "", 0, false, Vec::new(), 0)?;
    runtime.draw_composer()?;
    Ok(runtime)
}
