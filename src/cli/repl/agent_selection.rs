use super::session_support::reload_repl_agent;
use crate::agent::{Agent, AgentMode};
use crate::cli::agent_select;
use crate::cli::repl_runtime::ReplRuntime;
use crate::config::AppConfig;
use crate::control_commands::{run_agent_command, ControlSurface};
use crate::i18n::text as t;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use anyhow::Result;

/// 【终端】【Agent 状态】选择档案后需要同时刷新的配置、客户端和界面状态。
pub(super) struct AgentSelectionState<'a> {
    pub config: &'a mut AppConfig,
    pub client: &'a mut OpenAiCompatibleClient,
    pub agent: &'a mut Agent,
    pub runtime: &'a mut ReplRuntime,
}

/// 【终端】【Agent 选择】处理直接选择与交互选择，并在保存后刷新当前运行时。
/// @param paths 为存储路径；selection 为可选序号；state 为当前配置、客户端、Agent 和终端状态
/// @param mode 为权限模式；thinking_override 为命令行思考级别覆盖
/// @returns 交互与配置刷新结果；取消或选择错误展示提示后正常返回
pub(super) fn run_command(
    paths: &SaiPaths,
    selection: Option<usize>,
    state: AgentSelectionState<'_>,
    mode: AgentMode,
    thinking_override: Option<&str>,
) -> Result<()> {
    let AgentSelectionState {
        config,
        client,
        agent,
        runtime,
    } = state;
    // 1. 【终端】【Agent 选择】交互面板退出后重绘，清除占用行的残留
    let selection = match selection {
        Some(index) => Some(index),
        None => {
            let picked = agent_select::select_agent_index_interactively(paths);
            runtime.redraw()?;
            match picked {
                Ok(index) => index,
                Err(error) => {
                    runtime.record_meta(error.to_string())?;
                    return Ok(());
                }
            }
        }
    };
    let Some(selection) = selection else {
        runtime.record_meta(t("agent selection cancelled", "已取消 Agent 选择").to_string())?;
        return Ok(());
    };

    // 2. 【终端】【Agent 刷新】仅在配置确实变更时重建客户端与工具表
    match run_agent_command(paths, Some(selection), ControlSurface::Repl) {
        Ok(result) => {
            runtime.record_meta(result.message)?;
            if result.changed {
                reload_repl_agent(paths, config, client, agent, mode, thinking_override)?;
                runtime.record_meta(t("configuration reloaded", "配置已重新加载").to_string())?;
            }
        }
        Err(error) => runtime.record_meta(error.to_string())?,
    }
    Ok(())
}
