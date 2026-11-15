//! 流式交互完成后同步主循环配置，排队请求使用已保存的新设置。

use super::*;

/// 【终端设置】【空闲同步】参数为路径、主循环配置、客户端、空闲 Agent、界面和思考覆盖；返回重载结果
#[allow(clippy::too_many_arguments)]
pub(super) fn apply(
    paths: &SaiPaths,
    config: &mut AppConfig,
    client: &mut OpenAiCompatibleClient,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    mode: AgentMode,
    thinking_override: &mut Option<String>,
) -> Result<()> {
    let pending = runtime.take_pending_settings();
    if !pending.reload {
        return Ok(());
    }
    if pending.clear_thinking_override {
        *thinking_override = None;
    }
    reload_repl_agent(
        paths,
        config,
        client,
        agent,
        mode,
        thinking_override.as_deref(),
    )?;
    Ok(())
}
