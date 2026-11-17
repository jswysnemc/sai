//! Agent 档案管理菜单与分区编辑器。

mod permissions;
mod summary;

use permissions::{edit_agent_skills, edit_agent_tools};
#[cfg(test)]
use permissions::{initial_skill_state, initial_tool_state};
use summary::{basics_summary, profile_overview, prompt_summary, skills_summary, tools_summary};

use crate::config::{
    normalize_deferred_tools, AgentProfile, AppConfig, DEFAULT_AGENT_ID, DEFERRED_ALL_NON_BASE,
    EXPLORE_AGENT_ID, GATEWAY_AGENT_ID, GENERAL_AGENT_ID,
};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::collections::BTreeSet;
use std::io;

use super::form::{
    edit_textarea, parse_bool_field, parse_provider_model_choice, provider_model_choice_values,
    run_form, Field,
};
use super::mouse::{ListMouseState, MenuInput};
use super::multi_select::{run_multi_select, HeaderToggle, SelectEntry, StateStyle};
use super::ui::{draw_menu_interactive, message};

/// 【Agent配置】【列表管理】编辑统一 Agent 档案和各运行入口默认项，支持 `/` 搜索与粘贴过滤。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `paths`: Sai 路径（枚举工具与 Skills 目录）
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 编辑流程是否成功
pub(crate) fn edit_agents(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
) -> Result<()> {
    let mut selected = 0usize;
    let mut mouse = ListMouseState::default();
    let mut status = String::new();
    let mut search = super::search::ListSearch::default();
    loop {
        let profiles = visible_profiles(config);
        let shown = profiles
            .iter()
            .enumerate()
            .filter(|(_, profile)| {
                search.matches(&profile.name)
                    || search.matches(&profile.id)
                    || search.matches(&profile_overview(profile))
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let mut options = vec![t("Surface defaults", "入口默认 Agent").to_string()];
        if shown.is_empty() && !search.query.is_empty() {
            options.push(format!("({})", t("no matches", "没有匹配")));
        } else {
            options.extend(
                shown
                    .iter()
                    .map(|index| format!("{} [{}]", profiles[*index].name, profiles[*index].id)),
            );
        }
        options.push(t("Add Agent", "新增 Agent").to_string());
        let mut details = vec![t(
            "Choose which Agent profile each surface (Web / TUI / CLI) uses by default.",
            "为 Web / TUI / CLI 各入口选择默认 Agent 档案。",
        )
        .to_string()];
        if shown.is_empty() && !search.query.is_empty() {
            details
                .push(t("No Agent matches this filter.", "没有 Agent 命中当前过滤。").to_string());
        } else {
            details.extend(
                shown
                    .iter()
                    .map(|index| profile_overview(&profiles[*index])),
            );
        }
        details.push(
            t(
                "Create a new custom Agent profile and open its editor.",
                "新建自定义 Agent 档案并打开编辑器。",
            )
            .to_string(),
        );
        selected = selected.min(options.len().saturating_sub(1));
        let hitbox = draw_menu_interactive(
            stdout,
            t(" AGENTS ", " AGENT 配置 "),
            &options,
            &details,
            selected,
            &mouse,
            &search.help().unwrap_or_else(|| {
                if status.is_empty() {
                    super::theme::help_line(&[
                        ("/", t("search", "搜索")),
                        ("Enter", t("edit", "编辑")),
                        ("d", t("delete custom Agent", "删除自定义 Agent")),
                        ("q", t("back", "返回")),
                    ])
                } else {
                    status.clone()
                }
            }),
            "",
        )?;
        let key = match hitbox.read(&mut mouse, selected, false)? {
            MenuInput::Redraw => continue,
            MenuInput::Paste(text) => {
                if search.handle_paste(&text) == super::search::SearchEffect::Updated {
                    selected = 0;
                }
                continue;
            }
            MenuInput::Select(index) | MenuInput::RightClick(index) => {
                selected = index;
                continue;
            }
            MenuInput::Activate(index) => {
                selected = index;
                KeyCode::Enter
            }
            MenuInput::Action(code) => code,
            MenuInput::Key(key) => key.code,
        };
        match search.handle(key) {
            super::search::SearchEffect::Updated => {
                selected = 0;
                continue;
            }
            super::search::SearchEffect::Closed => continue,
            super::search::SearchEffect::Passthrough => {}
        }
        let profile_at = |row: usize| -> Option<usize> {
            row.checked_sub(1)
                .and_then(|index| shown.get(index).copied())
        };
        match key {
            KeyCode::Esc | KeyCode::Char('q') if !search.editing => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                selected = (selected + 1).min(options.len().saturating_sub(1))
            }
            KeyCode::Char('d') => {
                let Some(index) = profile_at(selected) else {
                    continue;
                };
                let id = profiles[index].id.clone();
                if !is_builtin(&id) {
                    if super::ui::confirm_delete(
                        stdout,
                        t(" DELETE AGENT ", " 删除自定义 Agent "),
                        &id,
                        t(
                            "Removes this Agent profile permanently.",
                            "永久删除该 Agent 档案，无法恢复。",
                        ),
                    )? {
                        config.agents.retain(|profile| profile.id != id);
                        selected = selected.saturating_sub(1);
                        status = format!("{}: {id}", t("Removed agent", "已删除 Agent"));
                    } else {
                        status = t("Delete cancelled", "已取消删除").to_string();
                    }
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
            KeyCode::Enter => {
                if let Some(index) = profile_at(selected) {
                    edit_agent_profile(stdout, paths, config, profiles[index].clone())?;
                }
            }
            _ => {}
        }
    }
}

/// 编辑 Web、TUI 与 CLI 默认 Agent。
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

/// 【Agent配置】【档案分区编辑】编辑基本信息、系统提示词、工具与 Skills 能力。
fn edit_agent_profile(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &mut AppConfig,
    mut profile: AgentProfile,
) -> Result<()> {
    let baseline = serde_json::to_string(&profile).ok();
    let mut unsaved_guard = super::input::UnsavedState::default();
    let mut selected = 0usize;
    let mut mouse = ListMouseState::default();
    loop {
        let dirty = serde_json::to_string(&profile).ok() != baseline;
        unsaved_guard.set(dirty);

        let options = vec![
            t("Basic info", "基本信息").to_string(),
            t("System prompt", "系统提示词").to_string(),
            t("Tool capabilities", "工具能力").to_string(),
            "Skills".to_string(),
            t("Save changes", "保存修改").to_string(),
        ];
        let details = vec![
            basics_summary(&profile),
            prompt_summary(&profile),
            tools_summary(&profile),
            skills_summary(&profile),
            t(
                "Write this profile into the in-memory config. Use Save & Exit on the main menu to persist to disk. q discards edits.",
                "把档案写入内存配置；主菜单「保存并退出」才会落盘。q 放弃本次修改。",
            )
            .to_string(),
        ];
        let subtitle = format!("{} [{}]", profile.name, profile.id);
        let hitbox = draw_menu_interactive(
            stdout,
            t(" EDIT AGENT ", " 编辑 AGENT "),
            &options,
            &details,
            selected,
            &mouse,
            &super::theme::help_line(&[
                ("Enter", t("open", "进入")),
                ("s", t("save", "保存")),
                ("q", t("discard", "放弃")),
            ]),
            &subtitle,
        )?;
        let code = match hitbox.read(&mut mouse, selected, false)? {
            MenuInput::Redraw | MenuInput::Paste(_) => continue,
            MenuInput::Select(index) | MenuInput::RightClick(index) => {
                selected = index;
                continue;
            }
            MenuInput::Activate(index) => {
                selected = index;
                KeyCode::Enter
            }
            MenuInput::Action(code) => code,
            MenuInput::Key(key) => key.code,
        };
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                selected = (selected + 1).min(options.len().saturating_sub(1))
            }
            KeyCode::Char('s') => {
                upsert_agent(config, profile);
                return Ok(());
            }
            KeyCode::Enter => match selected {
                0 => edit_agent_basics(stdout, config, &mut profile)?,
                1 => edit_textarea(stdout, &mut profile.system_prompt)?,
                2 => edit_agent_tools(stdout, paths, config, &mut profile)?,
                3 => edit_agent_skills(stdout, paths, config, &mut profile)?,
                4 => {
                    upsert_agent(config, profile);
                    return Ok(());
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// 编辑 Agent 基本信息（名称、描述、模型、思考等级与注册开关）。
fn edit_agent_basics(
    stdout: &mut io::Stdout,
    config: &AppConfig,
    profile: &mut AgentProfile,
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
    ];
    loop {
        if !run_form(stdout, t(" AGENT BASICS ", " AGENT 基本信息 "), &mut fields)? {
            return Ok(());
        }
        let (register_to_main, load_instruction_files) = match parse_bool_field(&fields[4].value)
            .and_then(|register| Ok((register, parse_bool_field(&fields[5].value)?)))
        {
            Ok(values) => values,
            Err(err) => {
                message(
                    stdout,
                    &format!("{}: {err}", t("Invalid input", "输入无效")),
                )?;
                continue;
            }
        };
        profile.name = fields[0].value.trim().to_string();
        profile.description = fields[1].value.trim().to_string();
        (profile.provider_id, profile.model) = parse_provider_model_choice(&fields[2].value);
        profile.thinking_level = fields[3].value.trim().to_string();
        profile.register_to_main = register_to_main;
        profile.load_instruction_files = load_instruction_files;
        return Ok(());
    }
}

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

fn optional_agent_id(value: &str) -> Option<String> {
    let value = value.trim();
    (value != DEFAULT_AGENT_ID && !value.is_empty()).then(|| value.to_string())
}

pub(super) fn is_builtin(id: &str) -> bool {
    matches!(
        id,
        DEFAULT_AGENT_ID | GENERAL_AGENT_ID | EXPLORE_AGENT_ID | GATEWAY_AGENT_ID
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_choices_include_default_and_builtins() {
        let choices = agent_choice_ids(&AppConfig::default());

        assert!(choices.contains(&DEFAULT_AGENT_ID.to_string()));
        assert!(choices.contains(&GENERAL_AGENT_ID.to_string()));
        assert!(choices.contains(&EXPLORE_AGENT_ID.to_string()));
        assert!(choices.contains(&GATEWAY_AGENT_ID.to_string()));
    }

    #[test]
    fn tool_state_reflects_inherit_and_deferred() {
        let mut profile = AgentProfile::default();
        assert_eq!(initial_tool_state(&profile, "read_file"), 1);

        profile.deferred_tools = vec!["show_meme".to_string()];
        assert_eq!(initial_tool_state(&profile, "show_meme"), 2);

        profile.enabled_tools = vec!["read_file".to_string(), "show_meme".to_string()];
        assert_eq!(initial_tool_state(&profile, "read_file"), 1);
        assert_eq!(initial_tool_state(&profile, "web_search"), 0);
    }

    #[test]
    fn skill_state_maps_full_and_named() {
        let profile = AgentProfile {
            skills_full: vec!["research".to_string()],
            skills_named: vec!["drawio".to_string()],
            ..AgentProfile::default()
        };

        assert_eq!(initial_skill_state(&profile, "research"), 1);
        assert_eq!(initial_skill_state(&profile, "drawio"), 2);
        assert_eq!(initial_skill_state(&profile, "other"), 0);
    }

    #[test]
    fn tools_summary_covers_modes() {
        let mut profile = AgentProfile::default();
        let inherit = tools_summary(&profile);
        assert!(
            inherit.contains("inherit all") || inherit.contains("继承全量"),
            "unexpected inherit summary: {inherit}"
        );

        profile.deferred_tools = vec![DEFERRED_ALL_NON_BASE.to_string()];
        assert!(tools_summary(&profile).contains('*'));

        profile.enabled_tools = vec!["read_file".to_string(), "grep".to_string()];
        profile.deferred_tools = vec!["grep".to_string()];
        let summary = tools_summary(&profile);
        assert!(summary.contains('2'));
        assert!(summary.contains('1'));
    }
}
