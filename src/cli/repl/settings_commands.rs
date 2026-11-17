use super::*;

/// 【终端】【配置命令】处理 `/config` 与 `/thinking`，完成后按新配置重建 Agent 并同步受管屏幕。
///
/// 参数:
/// - `input`: 提交文本
/// - `paths`: 存储路径
/// - `config`: 当前运行配置
/// - `client`: 模型客户端
/// - `agent`: 当前会话 Agent
/// - `runtime`: 终端运行期
/// - `mode`: 当前权限模式
/// - `thinking_override`: 命令行思考等级覆盖
///
/// 返回:
/// - 命中配置命令时返回 true；配置失败以提示行展示，不中断 REPL
#[allow(clippy::too_many_arguments)]
pub(super) fn handle(
    input: &str,
    paths: &SaiPaths,
    config: &mut AppConfig,
    client: &mut OpenAiCompatibleClient,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    mode: AgentMode,
    thinking_override: &mut Option<String>,
) -> Result<bool> {
    if input.eq_ignore_ascii_case("/config") {
        // 1. 【终端】【全屏备用屏】在全屏视图下直接嵌套共用备用屏，避免退回主屏闪烁
        command_screen::prepare_alternate_overlay(runtime)?;
        let result = crate::config_tui::run(paths);
        runtime.redraw()?;
        if let Err(err) = result {
            runtime.record_meta(err.to_string())?;
            return Ok(true);
        }
        reload_repl_agent(
            paths,
            config,
            client,
            agent,
            mode,
            thinking_override.as_deref(),
        )?;
        runtime.record_meta(t("configuration reloaded", "配置已重新加载").to_string())?;
        return Ok(true);
    }
    let Some(rest) =
        repl_command_rest(input, "/thinking").or_else(|| repl_command_rest(input, "/think"))
    else {
        return Ok(false);
    };
    let level = rest
        .split_whitespace()
        .next()
        .map(std::string::ToString::to_string);
    // 2. 【终端】【思考等级】通过静默接口更新，避免 println! 破坏受管屏幕；无参选择后重绘清除残留
    let result = crate::cli::providers::update_thinking_level(paths, level.as_deref(), false);
    if level.is_none() {
        runtime.redraw()?;
    }
    let status = match result {
        Ok(status) => status,
        Err(err) => {
            runtime.record_meta(err.to_string())?;
            return Ok(true);
        }
    };
    *thinking_override = None;
    reload_repl_agent(paths, config, client, agent, mode, None)?;
    runtime.record_meta(status)?;
    runtime.record_meta(t("configuration reloaded", "配置已重新加载").to_string())?;
    Ok(true)
}
