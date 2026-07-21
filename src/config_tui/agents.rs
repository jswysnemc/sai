use crate::config::{
    AgentProfile, AppConfig, DEFAULT_AGENT_ID, EXPLORE_AGENT_ID, GATEWAY_AGENT_ID, GENERAL_AGENT_ID,
};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io;

use super::form::{
    parse_bool_field, parse_provider_model_choice, provider_model_choice_values, run_form, Field,
};
use super::input::read_key;
use super::ui::draw_menu;

/// 编辑统一 Agent 档案和各运行入口默认项。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 编辑流程是否成功
pub(crate) fn edit_agents(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut selected = 0usize;
    loop {
        let profiles = visible_profiles(config);
        let mut options = vec![t("Surface defaults", "入口默认 Agent").to_string()];
        options.extend(
            profiles
                .iter()
                .map(|profile| format!("{} [{}]", profile.name, profile.id)),
        );
        options.push(t("Add Agent", "新增 Agent").to_string());
        draw_menu(
            stdout,
            t(" AGENTS ", " AGENT 配置 "),
            &options,
            selected,
            t(
                "Enter edit · d delete custom Agent · q back",
                "Enter 编辑 · d 删除自定义 Agent · q 返回",
            ),
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                selected = (selected + 1).min(options.len().saturating_sub(1))
            }
            KeyCode::Char('d') if selected > 0 && selected <= profiles.len() => {
                let id = &profiles[selected - 1].id;
                if !is_builtin(id) {
                    config.agents.retain(|profile| &profile.id != id);
                    selected = selected.saturating_sub(1);
                }
            }
            KeyCode::Enter if selected == 0 => edit_surface_defaults(stdout, config)?,
            KeyCode::Enter if selected == options.len() - 1 => {
                let profile = new_agent(config);
                let id = profile.id.clone();
                config.agents.push(profile);
                let profiles = visible_profiles(config);
                selected = profiles
                    .iter()
                    .position(|profile| profile.id == id)
                    .map(|index| index + 1)
                    .unwrap_or(0);
            }
            KeyCode::Enter if selected > 0 && selected <= profiles.len() => {
                edit_agent_profile(stdout, config, profiles[selected - 1].clone())?;
            }
            _ => {}
        }
    }
}

/// 编辑 Web、TUI 与 CLI 默认 Agent。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 表单编辑结果
fn edit_surface_defaults(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let choices = agent_choice_ids(config);
    let mut fields = vec![
        Field::new(
            t("Web default Agent", "Web 默认 Agent"),
            config
                .default_agent
                .clone()
                .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
        )
        .choices_owned(choices.clone()),
        Field::new(
            t("TUI default Agent", "TUI 默认 Agent"),
            config
                .tui_agent
                .clone()
                .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
        )
        .choices_owned(choices.clone()),
        Field::new(
            t("CLI default Agent", "CLI 默认 Agent"),
            config
                .cli_agent
                .clone()
                .unwrap_or_else(|| DEFAULT_AGENT_ID.to_string()),
        )
        .choices_owned(choices.clone()),
        Field::new(
            t("Gateway default Agent", "网关默认 Agent"),
            config
                .gateway_agent
                .clone()
                .unwrap_or_else(|| GATEWAY_AGENT_ID.to_string()),
        )
        .choices_owned(choices),
    ];
    if run_form(
        stdout,
        t(" AGENT DEFAULTS ", " AGENT 入口默认值 "),
        &mut fields,
    )? {
        config.default_agent = optional_agent_id(&fields[0].value);
        config.tui_agent = optional_agent_id(&fields[1].value);
        config.cli_agent = optional_agent_id(&fields[2].value);
        config.gateway_agent = optional_agent_id(&fields[3].value);
    }
    Ok(())
}

/// 编辑单个统一 Agent 档案。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
/// - `profile`: 当前 Agent 档案
///
/// 返回:
/// - 表单编辑结果
fn edit_agent_profile(
    stdout: &mut io::Stdout,
    config: &mut AppConfig,
    mut profile: AgentProfile,
) -> Result<()> {
    let model_value = if profile.provider_id.is_empty() || profile.model.is_empty() {
        String::new()
    } else {
        format!("{}\t{}", profile.provider_id, profile.model)
    };
    let mut fields = vec![
        Field::new(t("Display name", "显示名称"), profile.name.clone()),
        Field::new(t("Description", "用途描述"), profile.description.clone()),
        Field::new(t("Provider/model", "供应商/模型"), model_value)
            .choices_owned(provider_model_choice_values(config, false))
            .empty_choice_label(t("Inherit current model", "沿用当前模型")),
        Field::new(
            t("Thinking level", "思考等级"),
            profile.thinking_level.clone(),
        )
        .choices(&["auto", "none", "low", "medium", "high", "xhigh", "max"]),
        Field::boolean(
            t("Register to main Agent", "向主 Agent 注册"),
            profile.register_to_main,
        ),
        Field::boolean(
            t("Load AGENT.md instruction files", "加载 AGENT.md 指令文件"),
            profile.load_instruction_files,
        ),
        Field::textarea(
            t("System prompt", "系统提示词"),
            profile.system_prompt.clone(),
        ),
        Field::textarea(
            t("Enabled tools, one per line (empty = all)", "启用工具，每行一个（空=全量）"),
            profile.enabled_tools.join("\n"),
        ),
        Field::textarea(
            t("Full Skills, one per line", "完整 Skills，每行一个"),
            profile.skills_full.join("\n"),
        ),
        Field::textarea(
            t("Named Skills, one per line", "名称 Skills，每行一个"),
            profile.skills_named.join("\n"),
        ),
    ];
    if run_form(stdout, t(" EDIT AGENT ", " 编辑 AGENT "), &mut fields)? {
        profile.name = fields[0].value.trim().to_string();
        profile.description = fields[1].value.trim().to_string();
        (profile.provider_id, profile.model) = parse_provider_model_choice(&fields[2].value);
        profile.thinking_level = fields[3].value.trim().to_string();
        profile.register_to_main = parse_bool_field(&fields[4].value)?;
        profile.load_instruction_files = parse_bool_field(&fields[5].value)?;
        profile.system_prompt = fields[6].value.trim().to_string();
        profile.enabled_tools = parse_lines(&fields[7].value);
        profile.skills_full = parse_lines(&fields[8].value);
        profile.skills_named = parse_lines(&fields[9].value);
        upsert_agent(config, profile);
    }
    Ok(())
}

/// 返回 TUI 可编辑的默认、内置和自定义 Agent。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - Agent 档案列表
fn visible_profiles(config: &AppConfig) -> Vec<AgentProfile> {
    let mut profiles = config.resolved_agent_profiles();
    if !profiles
        .iter()
        .any(|profile| profile.id == DEFAULT_AGENT_ID)
    {
        profiles.insert(
            0,
            AgentProfile {
                id: DEFAULT_AGENT_ID.to_string(),
                name: t("Default Agent", "默认 Agent").to_string(),
                description: t("Inherit global configuration", "继承全局配置").to_string(),
                ..AgentProfile::default()
            },
        );
    }
    profiles
}

/// 创建不与现有标识冲突的自定义 Agent。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 新 Agent 档案
fn new_agent(config: &AppConfig) -> AgentProfile {
    let used = config
        .resolved_agent_profiles()
        .into_iter()
        .map(|profile| profile.id)
        .collect::<std::collections::HashSet<_>>();
    let mut index = 1usize;
    while used.contains(&format!("agent-{index}")) {
        index += 1;
    }
    AgentProfile {
        id: format!("agent-{index}"),
        name: format!("{} {index}", t("New Agent", "新 Agent")),
        thinking_level: "auto".to_string(),
        ..AgentProfile::default()
    }
}

/// 写入或替换指定 Agent 档案。
///
/// 参数:
/// - `config`: 待更新应用配置
/// - `profile`: Agent 档案
///
/// 返回:
/// - 无
fn upsert_agent(config: &mut AppConfig, profile: AgentProfile) {
    if let Some(existing) = config
        .agents
        .iter_mut()
        .find(|existing| existing.id == profile.id)
    {
        *existing = profile;
    } else {
        config.agents.push(profile);
    }
}

/// 返回所有入口默认项可以选择的 Agent 标识。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 去重后的 Agent 标识
fn agent_choice_ids(config: &AppConfig) -> Vec<String> {
    let mut ids = vec![DEFAULT_AGENT_ID.to_string()];
    ids.extend(
        config
            .resolved_agent_profiles()
            .into_iter()
            .map(|profile| profile.id),
    );
    ids.sort();
    ids.dedup();
    ids
}

/// 将默认 Agent 标识转换为空配置值。
///
/// 参数:
/// - `value`: 表单 Agent 标识
///
/// 返回:
/// - 非默认 Agent 标识
fn optional_agent_id(value: &str) -> Option<String> {
    let value = value.trim();
    (value != DEFAULT_AGENT_ID && !value.is_empty()).then(|| value.to_string())
}

/// 解析每行一个值的表单文本。
///
/// 参数:
/// - `value`: 多行文本
///
/// 返回:
/// - 去除空行后的值列表
fn parse_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// 判断 Agent 是否为不可删除的内置档案。
///
/// 参数:
/// - `id`: Agent 标识
///
/// 返回:
/// - 是否为内置档案
fn is_builtin(id: &str) -> bool {
    matches!(
        id,
        DEFAULT_AGENT_ID | GENERAL_AGENT_ID | EXPLORE_AGENT_ID | GATEWAY_AGENT_ID
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证入口默认选项包含虚拟默认项和内置 Agent。
    #[test]
    fn agent_choices_include_default_and_builtins() {
        let choices = agent_choice_ids(&AppConfig::default());

        assert!(choices.contains(&DEFAULT_AGENT_ID.to_string()));
        assert!(choices.contains(&GENERAL_AGENT_ID.to_string()));
        assert!(choices.contains(&EXPLORE_AGENT_ID.to_string()));
        assert!(choices.contains(&GATEWAY_AGENT_ID.to_string()));
    }

    /// 验证多行能力列表会去除空白项。
    #[test]
    fn parses_agent_capability_lines() {
        assert_eq!(parse_lines(" read_file \n\n grep\n"), ["read_file", "grep"]);
    }
}
