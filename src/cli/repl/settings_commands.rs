use super::*;

/// 【终端】【配置命令】处理 `/config` 与 `/thinking`，完成后按新配置重建 Agent。
///
/// 参数: `input` 为提交文本，其余为存储路径、当前配置、客户端、Agent、界面、权限模式与思考覆盖
/// 返回: 命中配置命令时返回 true；配置失败以提示行展示，不中断 REPL
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
        let result = crate::config_tui::run(paths);
        // 1. 全屏备用屏退出后整页重放，避免残留边框
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
    let Some(rest) = repl_command_rest(input, "/thinking") else {
        return Ok(false);
    };
    let level = rest
        .split_whitespace()
        .next()
        .map(std::string::ToString::to_string);
    let result = run_set_thinking(paths, SetThinkingArgs { level });
    // 2. 交互选择会占用内联区域，退出后重放清除残留
    if rest.trim().is_empty() {
        runtime.redraw()?;
    }
    if let Err(err) = result {
        runtime.record_meta(err.to_string())?;
        return Ok(true);
    }
    *thinking_override = None;
    reload_repl_agent(paths, config, client, agent, mode, None)?;
    runtime.record_meta(t("configuration reloaded", "配置已重新加载").to_string())?;
    Ok(true)
}
