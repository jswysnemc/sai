use crate::config::{AgentProfile, AppConfig};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};

use super::ControlSurface;

#[derive(Debug, Clone)]
pub struct AgentCommandResult {
    pub message: String,
    pub changed: bool,
}

/// 执行 Agent 控制命令。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `selection`: 可选 Agent 序号
/// - `surface`: 命令入口类型
///
/// 返回:
/// - 命令结果
pub fn run_agent_command(
    paths: &SaiPaths,
    selection: Option<usize>,
    surface: ControlSurface,
) -> Result<AgentCommandResult> {
    AppConfig::init_files(paths)?;
    let mut config = AppConfig::load(paths)?;
    let choices = agent_choices(&config);
    if choices.is_empty() {
        bail!("{}", t("no agents available", "没有可用的 Agent"));
    }

    match selection {
        Some(index) => switch_agent_by_index(paths, &mut config, &choices, index, surface),
        None => Ok(AgentCommandResult {
            message: format_agent_list(&config, &choices, surface),
            changed: false,
        }),
    }
}

/// 按序号切换当前入口默认 Agent。
fn switch_agent_by_index(
    paths: &SaiPaths,
    config: &mut AppConfig,
    choices: &[AgentProfile],
    index: usize,
    surface: ControlSurface,
) -> Result<AgentCommandResult> {
    if index == 0 || index > choices.len() {
        bail!("{}: {index}", t("agent index out of range", "Agent 序号超出范围"));
    }
    let choice = &choices[index - 1];
    match surface {
        ControlSurface::Repl => {
            config.tui_agent = Some(choice.id.clone());
        }
        ControlSurface::Gateway => {
            config.gateway_agent = Some(choice.id.clone());
        }
    }
    config.save(paths)?;
    Ok(AgentCommandResult {
        message: format!(
            "{}: {index}. {} ({})",
            t("active agent", "当前 Agent"),
            choice.name,
            choice.id
        ),
        changed: true,
    })
}

/// 可切换的 Agent 列表：内置 + 自定义，追加虚拟 default。
pub fn agent_choices(config: &AppConfig) -> Vec<AgentProfile> {
    let mut profiles = config.resolved_agent_profiles();
    // 虚拟 default：继承全局提示词/工具
    profiles.insert(
        0,
        AgentProfile {
            id: crate::config::DEFAULT_AGENT_ID.to_string(),
            name: "默认 Agent".to_string(),
            description: "继承当前全局配置（CLI 默认 Sai）".to_string(),
            register_to_main: false,
            ..AgentProfile::default()
        },
    );
    profiles
}

fn current_agent_id(config: &AppConfig, surface: ControlSurface) -> Option<&str> {
    match surface {
        ControlSurface::Repl => config.tui_agent.as_deref(),
        ControlSurface::Gateway => config.gateway_agent.as_deref(),
    }
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .or(Some(crate::config::DEFAULT_AGENT_ID))
}

fn format_agent_list(
    config: &AppConfig,
    choices: &[AgentProfile],
    surface: ControlSurface,
) -> String {
    let active = current_agent_id(config, surface).unwrap_or(crate::config::DEFAULT_AGENT_ID);
    let mut lines = vec![t("Available agents:", "可用 Agent:").to_string()];
    for (index, choice) in choices.iter().enumerate() {
        let marker = if choice.id == active { "*" } else { " " };
        let desc = if choice.description.trim().is_empty() {
            String::new()
        } else {
            format!(" — {}", choice.description.trim())
        };
        lines.push(format!(
            "{marker} {}. {} ({}){desc}",
            index + 1,
            choice.name,
            choice.id
        ));
    }
    lines.push(agent_switch_hint(surface));
    lines.join("\n")
}

fn agent_switch_hint(surface: ControlSurface) -> String {
    if surface == ControlSurface::Gateway {
        t(
            "Use /agent <index> or /代理 <序号> to switch.",
            "使用 /agent <序号> 或 /代理 <序号> 切换。",
        )
        .to_string()
    } else {
        t(
            "Use /agent to pick interactively, or /agent <index>.",
            "使用 /agent 交互选择，或 /agent <序号>。",
        )
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_choices_include_default_and_gateway() {
        let config = AppConfig::default();
        let choices = agent_choices(&config);
        assert!(choices.iter().any(|item| item.id == "default"));
        assert!(choices.iter().any(|item| item.id == "gateway"));
        assert!(choices.iter().any(|item| item.id == "general"));
    }
}
